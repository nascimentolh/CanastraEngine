//! Rectangles and client textures: one instanced pipeline, one draw call per run of the same texture.

use std::collections::HashMap;
use std::ops::Range;

use canastra_ui::Draw;
use l2_catalog::Catalog;
use wgpu::util::{DeviceExt, TextureDataOrder};

use super::quad::{QUAD_BYTES, nine_slice, shadow, shape};

struct Image {
    group: wgpu::BindGroup,
    size: [f32; 2],
}

pub(super) struct Shapes {
    pipeline: wgpu::RenderPipeline,
    globals_layout: wgpu::BindGroupLayout,
    image_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    globals: wgpu::Buffer,
    quads: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    /// Bound for untextured quads; the shader always samples something.
    blank: wgpu::BindGroup,
    catalog: Catalog,
    /// Lower-case texture path to its GPU image, `None` when the client does not have it.
    images: HashMap<String, Option<Image>>,
    /// Instance ranges of the last prepared frame with the texture they sample, and how many of them draw under
    /// the overlays.
    batches: Vec<(Option<String>, Range<u32>)>,
    base_batches: usize,
}

impl Shapes {
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        catalog: Catalog,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("ui.wgsl"));
        let buffer = |binding, ty| {
            let ty = wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None };
            wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None }
        };
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui globals"),
            entries: &[
                buffer(0, wgpu::BufferBindingType::Uniform),
                buffer(1, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let fragment = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        };
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui image"),
            entries: &[
                fragment(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                fragment(1, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui"),
            bind_group_layouts: &[Some(&globals_layout), Some(&image_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quads = quad_buffer(device, 64);
        let globals_group = globals_group(device, &globals_layout, &globals, &quads);
        let blank = image_group(device, queue, &image_layout, &sampler, 1, 1, &[255; 4]);
        Self {
            pipeline,
            globals_layout,
            image_layout,
            sampler,
            globals,
            quads,
            globals_group,
            blank,
            catalog,
            images: HashMap::new(),
            batches: Vec::new(),
            base_batches: 0,
        }
    }

    /// Uploads the rectangles and images of `draws` (logical pixels) for a target of `size` physical pixels; draws
    /// from `overlay_from` on make the overlay layer.
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        scale: f32,
        draws: &[Draw],
        overlay_from: usize,
    ) {
        let mut bytes = Vec::new();
        self.batches.clear();
        self.base_batches = 0;
        for (index, draw) in draws.iter().enumerate() {
            if index == overlay_from {
                self.base_batches = self.batches.len();
            }
            let (key, quads) = match draw {
                Draw::Shadow { rect, radius, shadow: layer } => (None, vec![shadow(*rect, *radius, *layer, scale)]),
                Draw::Rect { rect, fill, border, radius } => (None, vec![shape(*rect, *fill, *border, *radius, scale)]),
                Draw::Image { rect, source, inset } => {
                    let key = source.to_lowercase();
                    let Some(image) = self.image(device, queue, &key) else { continue };
                    (Some(key), nine_slice(*rect, image.size, *inset, scale))
                }
                Draw::Text { .. } => continue,
            };
            let start = (bytes.len() as u64 / QUAD_BYTES) as u32;
            for quad in &quads {
                quad.write(&mut bytes);
            }
            let end = start + quads.len() as u32;
            // A run of the same texture never crosses into the overlays.
            let same_layer = self.batches.len() > self.base_batches;
            match self.batches.last_mut() {
                Some((last, range)) if *last == key && same_layer => range.end = end,
                _ => self.batches.push((key, start..end)),
            }
        }
        if overlay_from >= draws.len() {
            self.base_batches = self.batches.len();
        }
        if self.quads.size() < bytes.len() as u64 {
            self.quads = quad_buffer(device, bytes.len() / QUAD_BYTES as usize * 2);
            self.globals_group = globals_group(device, &self.globals_layout, &self.globals, &self.quads);
        }
        queue.write_buffer(&self.quads, 0, &bytes);
        let viewport = [size[0] as f32, size[1] as f32, 0.0, 0.0];
        queue.write_buffer(
            &self.globals,
            0,
            &viewport.iter().flat_map(|value| value.to_le_bytes()).collect::<Vec<_>>(),
        );
    }

    /// Draws the prepared shapes under the overlays, or with `overlay` those of the overlays.
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, overlay: bool) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.globals_group, &[]);
        let (base, overlays) = self.batches.split_at(self.base_batches.min(self.batches.len()));
        for (key, range) in if overlay { overlays } else { base } {
            let image = key.as_ref().and_then(|key| self.images.get(key)).and_then(Option::as_ref);
            pass.set_bind_group(1, image.map_or(&self.blank, |image| &image.group), &[]);
            pass.draw(0..4, range.clone());
        }
    }

    fn image(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, key: &str) -> Option<&Image> {
        if !self.images.contains_key(key) {
            let image = self.catalog.texture(key).map(|decoded| Image {
                group: image_group(
                    device,
                    queue,
                    &self.image_layout,
                    &self.sampler,
                    decoded.width,
                    decoded.height,
                    &decoded.rgba,
                ),
                size: [decoded.width as f32, decoded.height as f32],
            });
            if image.is_none() {
                eprintln!("texture not found: {key}");
            }
            self.images.insert(key.to_owned(), image);
        }
        self.images.get(key)?.as_ref()
    }
}

fn quad_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ui quads"),
        size: capacity.max(1) as u64 * QUAD_BYTES,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn globals_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    quads: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ui globals"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: quads.as_entire_binding() },
        ],
    })
}

fn image_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::BindGroup {
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("ui image"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        TextureDataOrder::LayerMajor,
        rgba,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ui image"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

//! GPU state for scene geometry: render pipelines per blend and depth setup, the bind group layouts and
//! texture and material upload.

use std::collections::HashMap;

use l2_catalog::Blend;
use ue2_assets::Image;
use wgpu::BlendFactor::{Dst, One, OneMinusSrc, Src, Zero};
use wgpu::util::{DeviceExt, TextureDataOrder};

pub(super) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Position (3 floats), UV (2 floats) and RGBA color (4 floats).
const VERTEX_BYTES: u64 = 36;
/// Six `vec4<f32>`, see `Material` in `scene.wgsl`.
pub(super) const MATERIAL_BYTES: u64 = 96;

/// How a batch meets the depth buffer and the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Draw {
    pub(super) blend: Blend,
    /// Hidden behind nearer surfaces.
    pub(super) depth_test: bool,
    /// Hides farther surfaces drawn later.
    pub(super) depth_write: bool,
}

impl Draw {
    /// Level geometry: opaque, masked and alpha surfaces write depth; other blends only test it.
    pub(super) fn surface(blend: Blend) -> Self {
        Self { blend, depth_test: true, depth_write: matches!(blend, Blend::Opaque | Blend::Masked | Blend::Alpha) }
    }
}

pub(super) struct Pipeline {
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    pipelines: HashMap<Draw, wgpu::RenderPipeline>,
    pub(super) globals: wgpu::BindGroupLayout,
    materials: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Pipeline {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        let uniform = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let globals = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene globals"),
            entries: &[entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, uniform)],
        });
        let fragment = wgpu::ShaderStages::FRAGMENT;
        let materials = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene material"),
            entries: &[
                entry(0, fragment, uniform),
                entry(1, fragment, texture),
                entry(2, fragment, texture),
                entry(3, fragment, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&globals), Some(&materials)],
            immediate_size: 0,
        });
        Self {
            shader: device.create_shader_module(wgpu::include_wgsl!("scene.wgsl")),
            layout,
            format,
            pipelines: HashMap::new(),
            globals,
            materials,
            // Unreal textures tile.
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    /// Builds the pipeline for `draw` if it does not exist yet.
    pub(super) fn prepare(&mut self, device: &wgpu::Device, draw: Draw) {
        if self.pipelines.contains_key(&draw) {
            return;
        }
        let color = |src_factor, dst_factor| wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor, dst_factor, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent::OVER,
        };
        let blend = match draw.blend {
            Blend::Opaque | Blend::Masked => None,
            Blend::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
            // Fermata, from the original client: Translucent is a screen blend.
            // ponytail: Brighten keeps the same factors until evidence of its own shows up.
            Blend::Translucent | Blend::Brighten => Some(color(One, OneMinusSrc)),
            // Unreal's modulate doubles: source times destination, twice.
            Blend::Modulate => Some(color(Dst, Src)),
            Blend::Darken => Some(color(Zero, OneMinusSrc)),
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene"),
            layout: Some(&self.layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4],
                })],
            },
            // ponytail: no culling; two-sided materials and mirrored actors look right without it.
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(draw.depth_write),
                // Reverse Z: nearer is larger. Equal passes so layers sharing a surface all draw.
                depth_compare: Some(if draw.depth_test {
                    wgpu::CompareFunction::GreaterEqual
                } else {
                    wgpu::CompareFunction::Always
                }),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.insert(draw, pipeline);
    }

    /// The pipeline for `draw`, which `prepare` must have built.
    pub(super) fn get(&self, draw: Draw) -> Option<&wgpu::RenderPipeline> {
        self.pipelines.get(&draw)
    }

    /// Uploads `image` and its mips as gamma-space texels, which the scene blends as they are.
    pub(super) fn texture(device: &wgpu::Device, queue: &wgpu::Queue, image: &Image) -> wgpu::TextureView {
        let (levels, texels) = super::mips::chain(image.width, image.height, &image.rgba);
        device
            .create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("scene texture"),
                    size: wgpu::Extent3d { width: image.width, height: image.height, depth_or_array_layers: 1 },
                    mip_level_count: levels,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                TextureDataOrder::LayerMajor,
                &texels,
            )
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// A material's uniform buffer and its bind group with both stage textures.
    pub(super) fn material(
        &self,
        device: &wgpu::Device,
        base: &wgpu::TextureView,
        layer: &wgpu::TextureView,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene material"),
            size: MATERIAL_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene material"),
            layout: &self.materials,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(base) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(layer) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        (uniform, group)
    }
}

/// The shader's `Material` uniform for `material` at `time` seconds; `fogged` false keeps fog off it.
pub(super) fn material_uniform(material: &l2_catalog::Material, time: f32, fogged: bool) -> Vec<u8> {
    use l2_catalog::{Combine, IDENTITY, UvMatrix};
    let rows = |[u, v]: UvMatrix| [[u[0], u[1], u[2], 0.0], [v[0], v[1], v[2], 0.0]];
    let (layer, combine, factor) = match &material.layer {
        Some((stage, Combine::Multiply, factor)) => (stage.matrix(time), 1.0, *factor),
        Some((stage, Combine::Add, factor)) => (stage.matrix(time), 2.0, *factor),
        Some((stage, Combine::Mask, factor)) => (stage.matrix(time), 3.0, *factor),
        Some((stage, Combine::AddMasked, factor)) => (stage.matrix(time), 4.0, *factor),
        None => (IDENTITY, 0.0, 1.0),
    };
    // How the shader fogs, fades and outputs the batch; see `params` in `scene.wgsl`. Ten more marks a
    // batch fog does not touch.
    let fog = match material.blend {
        Blend::Opaque | Blend::Masked | Blend::Alpha => 0.0,
        Blend::Translucent | Blend::Brighten => 1.0,
        Blend::Modulate => 2.0,
        Blend::Darken => 3.0,
    } + if fogged { 0.0 } else { 10.0 };
    let cutoff = match material.blend {
        Blend::Masked => material.alpha_ref.map_or(0.5, |alpha_ref| f32::from(alpha_ref) / 255.0),
        // Fully transparent texels would still write depth over what lies behind them.
        Blend::Alpha => 0.02,
        _ => 0.0,
    };
    let [base_u, base_v] = rows(material.base.matrix(time));
    let [layer_u, layer_v] = rows(layer);
    let color = material.color.map(|channel| f32::from(channel) / 255.0);
    let bytes: Vec<u8> = [base_u, base_v, layer_u, layer_v, color, [combine, factor, cutoff, fog]]
        .iter()
        .flatten()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    debug_assert_eq!(bytes.len() as u64, MATERIAL_BYTES);
    bytes
}

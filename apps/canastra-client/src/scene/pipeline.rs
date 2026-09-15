//! GPU state for scene geometry: one render pipeline per blend mode, the bind group layouts and
//! texture and material upload.

use l2_catalog::Blend;
use ue2_assets::Image;
use wgpu::BlendFactor::{Dst, One, OneMinusSrc, Src};
use wgpu::util::{DeviceExt, TextureDataOrder};

pub(super) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Position (3 floats) and UV (2 floats).
const VERTEX_BYTES: u64 = 20;
/// Six `vec4<f32>`, see `Material` in `scene.wgsl`.
pub(super) const MATERIAL_BYTES: u64 = 96;

pub(super) struct Pipeline {
    opaque: wgpu::RenderPipeline,
    alpha: wgpu::RenderPipeline,
    additive: wgpu::RenderPipeline,
    modulate: wgpu::RenderPipeline,
    brighten: wgpu::RenderPipeline,
    pub(super) globals: wgpu::BindGroupLayout,
    materials: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Pipeline {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("scene.wgsl"));
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
        let pipeline = |blend: Option<wgpu::BlendState>, depth_write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("scene"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: VERTEX_BYTES,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
                    })],
                },
                // ponytail: no culling; two-sided materials and mirrored actors look right without it.
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_write),
                    // Reverse Z: nearer is larger. Equal passes so layers sharing a surface all draw.
                    depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let color = |src_factor, dst_factor| wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor, dst_factor, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent::OVER,
        };
        Self {
            opaque: pipeline(None, true),
            alpha: pipeline(Some(wgpu::BlendState::ALPHA_BLENDING), true),
            additive: pipeline(Some(color(One, One)), false),
            // Unreal's modulate doubles: source times destination, twice.
            modulate: pipeline(Some(color(Dst, Src)), false),
            brighten: pipeline(Some(color(One, OneMinusSrc)), false),
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

    pub(super) fn for_blend(&self, blend: Blend) -> &wgpu::RenderPipeline {
        match blend {
            Blend::Opaque | Blend::Masked => &self.opaque,
            Blend::Alpha => &self.alpha,
            Blend::Additive => &self.additive,
            Blend::Modulate => &self.modulate,
            Blend::Brighten => &self.brighten,
        }
    }

    /// Uploads `image` and its mips as gamma-space texels; the shader converts after combining.
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

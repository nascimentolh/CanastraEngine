//! The 3D scene behind the UI: static geometry drawn from a fixed camera with animated materials.

mod camera;
mod load;
mod pipeline;

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::time::Instant;

use l2_catalog::{Blend, Combine, IDENTITY, Material, UvMatrix};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::gpu::Gpu;
use pipeline::{DEPTH_FORMAT, MATERIAL_BYTES, Pipeline};

/// Horizontal field of view in degrees, measured from where the moon and the tree fall in an H5 login
/// screenshot at a 1.9 aspect ratio.
// ponytail: fixed horizontal FOV; if other aspect ratios frame differently from H5, fix the vertical one instead.
const FOV: f32 = 50.0;

struct Batch {
    material: Material,
    uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
    indices: Range<u32>,
}

pub(crate) struct Scene {
    pipeline: Pipeline,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    batches: Vec<Batch>,
    rotation: [i32; 3],
    /// Zero of the clock materials animate on.
    started: Instant,
    /// Depth target and the size it was made for.
    depth: Option<(wgpu::TextureView, [u32; 2])>,
}

impl Scene {
    /// Loads `map` from the client, framed by the scene tagged `camera_tag`.
    pub(crate) fn load(gpu: &Gpu, client_root: &Path, map: &str, camera_tag: &str) -> Result<Self, String> {
        let data = load::load(client_root, map, camera_tag)?;
        let (device, queue) = (&gpu.device, &gpu.queue);
        let pipeline = Pipeline::new(device, gpu.config.format);
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene globals"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene globals"),
            layout: &pipeline.globals,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let vertices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("scene vertices"),
            contents: &data.vertices.iter().flatten().flat_map(|value| value.to_le_bytes()).collect::<Vec<_>>(),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("scene indices"),
            contents: &data.indices.iter().flat_map(|index| index.to_le_bytes()).collect::<Vec<_>>(),
            usage: wgpu::BufferUsages::INDEX,
        });
        let views: HashMap<&str, wgpu::TextureView> = data
            .textures
            .iter()
            .map(|(path, image)| (path.as_str(), Pipeline::texture(device, queue, image)))
            .collect();
        let batches: Vec<Batch> = data
            .batches
            .into_iter()
            .filter_map(|batch| {
                let base = views.get(batch.material.base.texture.as_str())?;
                // A material without a second stage samples its base twice; the shader ignores it.
                let layer = batch.material.layer.as_ref().and_then(|(stage, _, _)| views.get(stage.texture.as_str()));
                let (uniform, group) = pipeline.material(device, base, layer.unwrap_or(base));
                Some(Batch { material: batch.material, uniform, group, indices: batch.indices })
            })
            .collect();
        println!(
            "scene: {map} from {camera_tag}, {} vertices, {} triangles, {} materials, {} textures",
            data.vertices.len(),
            data.indices.len() / 3,
            batches.len(),
            views.len()
        );
        Ok(Self {
            pipeline,
            globals,
            globals_group,
            vertices,
            indices,
            batches,
            rotation: data.camera.rotation,
            started: Instant::now(),
            depth: None,
        })
    }

    /// Clears the window's `target` and draws the scene into it as it looks now.
    pub(crate) fn draw(&mut self, gpu: &Gpu, target: &wgpu::TextureView) -> wgpu::CommandBuffer {
        let size = gpu.size();
        let aspect = size[0] as f32 / size[1].max(1) as f32;
        let matrix = camera::view_projection(self.rotation, FOV, aspect);
        let bytes: Vec<u8> = matrix.iter().flatten().flat_map(|value| value.to_le_bytes()).collect();
        gpu.queue.write_buffer(&self.globals, 0, &bytes);
        let time = self.started.elapsed().as_secs_f32();
        for batch in &self.batches {
            gpu.queue.write_buffer(&batch.uniform, 0, &material_bytes(&batch.material, time));
        }
        if self.depth.as_ref().is_none_or(|(_, depth_size)| *depth_size != size) {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("scene depth"),
                size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.depth = Some((texture.create_view(&wgpu::TextureViewDescriptor::default()), size));
        }
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("scene") });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: self.depth.as_ref().map(|(view, _)| wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.globals_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        for batch in &self.batches {
            pass.set_pipeline(self.pipeline.for_blend(batch.material.blend));
            pass.set_bind_group(1, &batch.group, &[]);
            pass.draw_indexed(batch.indices.clone(), 0, 0..1);
        }
        drop(pass);
        encoder.finish()
    }
}

/// The shader's `Material` uniform for `material` at `time` seconds.
fn material_bytes(material: &Material, time: f32) -> Vec<u8> {
    let rows = |[u, v]: UvMatrix| [[u[0], u[1], u[2], 0.0], [v[0], v[1], v[2], 0.0]];
    let (layer, combine, factor) = match &material.layer {
        Some((stage, Combine::Multiply, factor)) => (stage.matrix(time), 1.0, *factor),
        Some((stage, Combine::Add, factor)) => (stage.matrix(time), 2.0, *factor),
        None => (IDENTITY, 0.0, 1.0),
    };
    let cutoff = match material.blend {
        Blend::Masked => 0.5,
        // Fully transparent texels would still write depth over what lies behind them.
        Blend::Alpha => 0.02,
        _ => 0.0,
    };
    let [base_u, base_v] = rows(material.base.matrix(time));
    let [layer_u, layer_v] = rows(layer);
    let color = material.color.map(|channel| f32::from(channel) / 255.0);
    let bytes: Vec<u8> = [base_u, base_v, layer_u, layer_v, color, [combine, factor, cutoff, 0.0]]
        .iter()
        .flatten()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    debug_assert_eq!(bytes.len() as u64, MATERIAL_BYTES);
    bytes
}

//! The 3D scene behind the UI: static geometry drawn from a fixed camera.

mod camera;
mod load;
mod pipeline;

use std::ops::Range;
use std::path::Path;

use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::gpu::Gpu;
use pipeline::{DEPTH_FORMAT, Pipeline};

/// Horizontal field of view in degrees, measured from where the moon and the tree fall in an H5 login
/// screenshot at a 1.9 aspect ratio.
// ponytail: fixed horizontal FOV; if other aspect ratios frame differently from H5, fix the vertical one instead.
const FOV: f32 = 50.0;

pub(crate) struct Scene {
    pipeline: Pipeline,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    batches: Vec<(wgpu::BindGroup, Range<u32>)>,
    rotation: [i32; 3],
    /// Depth target and the size it was made for.
    depth: Option<(wgpu::TextureView, [u32; 2])>,
}

impl Scene {
    /// Loads `map` from the client, framed by the scene tagged `camera_tag`.
    pub(crate) fn load(gpu: &Gpu, client_root: &Path, map: &str, camera_tag: &str) -> Result<Self, String> {
        let data = load::load(client_root, map, camera_tag)?;
        let device = &gpu.device;
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
        let batches = data
            .batches
            .iter()
            .map(|batch| (pipeline.texture(device, &gpu.queue, &batch.texture), batch.indices.clone()))
            .collect();
        println!(
            "scene: {map} from {camera_tag}, {} vertices, {} triangles, {} textures",
            data.vertices.len(),
            data.indices.len() / 3,
            data.batches.len()
        );
        Ok(Self {
            pipeline,
            globals,
            globals_group,
            vertices,
            indices,
            batches,
            rotation: data.camera.rotation,
            depth: None,
        })
    }

    /// Clears the window's `target` and draws the scene into it.
    pub(crate) fn draw(&mut self, gpu: &Gpu, target: &wgpu::TextureView) -> wgpu::CommandBuffer {
        let size = gpu.size();
        let aspect = size[0] as f32 / size[1].max(1) as f32;
        let matrix = camera::view_projection(self.rotation, FOV, aspect);
        let bytes: Vec<u8> = matrix.iter().flatten().flat_map(|value| value.to_le_bytes()).collect();
        gpu.queue.write_buffer(&self.globals, 0, &bytes);
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
        pass.set_pipeline(&self.pipeline.render);
        pass.set_bind_group(0, &self.globals_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        for (group, indices) in &self.batches {
            pass.set_bind_group(1, group, &[]);
            pass.draw_indexed(indices.clone(), 0, 0..1);
        }
        drop(pass);
        encoder.finish()
    }
}

//! The 3D scene behind the UI: level geometry with animated materials and sprite particles, drawn
//! from a fixed camera.

mod camera;
mod curves;
mod load;
mod mips;
mod particles;
mod pipeline;
mod terrain;

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::time::Instant;

use l2_catalog::{Material, Stage};
use ue2_level::Fog;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::gpu::Gpu;
use load::Vertex;
use particles::System;
use pipeline::{DEPTH_FORMAT, Draw, Pipeline, material_uniform};

/// Horizontal field of view in degrees, measured from where the moon and the tree fall in an H5 login
/// screenshot at a 1.9 aspect ratio.
// ponytail: fixed horizontal FOV; if other aspect ratios frame differently from H5, fix the vertical one instead.
const FOV: f32 = 50.0;
/// View-projection matrix, fog color and fog start and end; see `Globals` in `scene.wgsl`.
const GLOBALS_BYTES: u64 = 96;

/// Geometry drawn with one material: a range of level indices or of particle quad indices.
struct Batch {
    material: Material,
    draw: Draw,
    fogged: bool,
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
    systems: Vec<System>,
    /// One batch per particle system, in the same order.
    sprite_batches: Vec<Batch>,
    particle_vertices: wgpu::Buffer,
    particle_indices: wgpu::Buffer,
    rotation: [i32; 3],
    fog: Option<Fog>,
    /// Zero of the clock materials and particles run on.
    started: Instant,
    /// Depth target and the size it was made for.
    depth: Option<(wgpu::TextureView, [u32; 2])>,
}

impl Scene {
    /// Loads `map` from the client, framed by the scene tagged `camera_tag`.
    pub(crate) fn load(gpu: &Gpu, client_root: &Path, map: &str, camera_tag: &str) -> Result<Self, String> {
        let data = load::load(client_root, map, camera_tag)?;
        let (device, queue) = (&gpu.device, &gpu.queue);
        let mut pipeline = Pipeline::new(device, gpu.config.format);
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene globals"),
            size: GLOBALS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene globals"),
            layout: &pipeline.globals,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let views: HashMap<&str, wgpu::TextureView> = data
            .textures
            .iter()
            .map(|(path, image)| (path.as_str(), Pipeline::texture(device, queue, image)))
            .collect();
        let mut batch = |material: Material, draw: Draw, fogged: bool, indices: Range<u32>| {
            let base = views.get(material.base.texture.as_str())?;
            // A material without a second stage samples its base twice; the shader ignores it.
            let layer = material.layer.as_ref().and_then(|(stage, _, _)| views.get(stage.texture.as_str()));
            let (uniform, group) = pipeline.material(device, base, layer.unwrap_or(base));
            pipeline.prepare(device, draw);
            Some(Batch { material, draw, fogged, uniform, group, indices })
        };
        let batches: Vec<Batch> = data
            .batches
            .into_iter()
            .filter_map(|level| batch(level.material.clone(), Draw::surface(level.material.blend), true, level.indices))
            .collect();

        let mut systems = particles::start(&data.emitters, data.camera.location);
        systems.retain(|system| system.sprite.texture.as_deref().is_some_and(|path| views.contains_key(path)));
        let mut quads = 0;
        let mut sprite_batches = Vec::new();
        for system in &systems {
            let sprite = &system.sprite;
            let material = Material {
                base: Stage { texture: sprite.texture.clone().unwrap_or_default(), uv: Vec::new() },
                layer: None,
                blend: particles::blend(sprite.draw_style),
                color: [255; 4],
            };
            let draw = Draw { blend: material.blend, depth_test: sprite.z_test, depth_write: false };
            let start = u32::try_from(quads * 6).map_err(|_| "too many particles")?;
            quads += system.len();
            let end = u32::try_from(quads * 6).map_err(|_| "too many particles")?;
            sprite_batches.extend(batch(material, draw, sprite.fogged, start..end));
        }

        let vertices = buffer(device, "scene vertices", wgpu::BufferUsages::VERTEX, &vertex_bytes(&data.vertices));
        let indices = buffer(device, "scene indices", wgpu::BufferUsages::INDEX, &index_bytes(&data.indices));
        let particle_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle vertices"),
            size: (quads.max(1) * 4 * size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quad_indices: Vec<u32> = (0..u32::try_from(quads.max(1)).map_err(|_| "too many particles")?)
            .flat_map(|quad| [0, 1, 2, 0, 2, 3].map(|corner| quad * 4 + corner))
            .collect();
        let particle_indices =
            buffer(device, "particle indices", wgpu::BufferUsages::INDEX, &index_bytes(&quad_indices));
        println!(
            "scene: {map} from {camera_tag}, {} vertices, {} triangles, {} materials, {} textures, {} particle systems with {quads} particles",
            data.vertices.len(),
            data.indices.len() / 3,
            batches.len(),
            views.len(),
            systems.len(),
        );
        Ok(Self {
            pipeline,
            globals,
            globals_group,
            vertices,
            indices,
            batches,
            systems,
            sprite_batches,
            particle_vertices,
            particle_indices,
            rotation: data.camera.rotation,
            fog: data.fog,
            started: Instant::now(),
            depth: None,
        })
    }

    /// Clears the window's `target` and draws the scene into it as it looks now.
    pub(crate) fn draw(&mut self, gpu: &Gpu, target: &wgpu::TextureView) -> wgpu::CommandBuffer {
        let size = gpu.size();
        let aspect = size[0] as f32 / size[1].max(1) as f32;
        let matrix = camera::view_projection(self.rotation, FOV, aspect);
        // Without fog, the range starts beyond any distance drawn.
        let (fog_color, fog_range) = self.fog.map_or(([0.0; 4], [f32::MAX, f32::MAX, 0.0, 0.0]), |fog| {
            (fog.color.map(|channel| f32::from(channel) / 255.0), [fog.start, fog.end, 0.0, 0.0])
        });
        let bytes: Vec<u8> =
            matrix.iter().chain([&fog_color, &fog_range]).flatten().flat_map(|value| value.to_le_bytes()).collect();
        gpu.queue.write_buffer(&self.globals, 0, &bytes);
        let time = self.started.elapsed().as_secs_f32();
        for batch in self.batches.iter().chain(&self.sprite_batches) {
            gpu.queue.write_buffer(&batch.uniform, 0, &material_uniform(&batch.material, time, batch.fogged));
        }
        for system in &mut self.systems {
            system.update(time);
        }
        let mut sprites = Vec::new();
        for system in &self.systems {
            system.quads(time, self.rotation, &mut sprites);
        }
        gpu.queue.write_buffer(&self.particle_vertices, 0, &vertex_bytes(&sprites));
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
        // Level geometry first, then particles over it.
        // ponytail: particle systems draw in level order, not sorted by distance; sort them if overlaps show.
        for (batches, vertices, indices) in [
            (&self.batches, &self.vertices, &self.indices),
            (&self.sprite_batches, &self.particle_vertices, &self.particle_indices),
        ] {
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            for batch in batches {
                let Some(pipeline) = self.pipeline.get(batch.draw) else { continue };
                pass.set_pipeline(pipeline);
                pass.set_bind_group(1, &batch.group, &[]);
                pass.draw_indexed(batch.indices.clone(), 0, 0..1);
            }
        }
        drop(pass);
        encoder.finish()
    }
}

fn buffer(device: &wgpu::Device, label: &str, usage: wgpu::BufferUsages, contents: &[u8]) -> wgpu::Buffer {
    device.create_buffer_init(&BufferInitDescriptor { label: Some(label), contents, usage })
}

fn vertex_bytes(vertices: &[Vertex]) -> Vec<u8> {
    vertices.iter().flatten().flat_map(|value| value.to_le_bytes()).collect()
}

fn index_bytes(indices: &[u32]) -> Vec<u8> {
    indices.iter().flat_map(|index| index.to_le_bytes()).collect()
}

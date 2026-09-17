//! The 3D scene behind the UI: level geometry with animated materials and sprite particles, drawn
//! from a fixed camera.

mod bsp;
mod camera;
mod cast;
mod curves;
mod daylight;
mod deco;
mod emission;
mod flight;
mod load;
mod mips;
mod movers;
mod particle_mesh;
mod particles;
mod pawns;
mod pipeline;
mod random;
mod sky;
mod terrain;

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::path::Path;
use std::time::Instant;

use l2_catalog::{Catalog, Material, UvModifier};
use ue2_level::{Placement, Shot, Warp};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::gpu::Gpu;
pub(crate) use flight::Route;
use load::Vertex;
use particles::System;
use pawns::Pawn;
pub(crate) use pawns::{Figure, HeldSource, PartSource};
use pipeline::{Draw, Pipeline, depth_texture, material_buffer, material_uniform, target};

/// Horizontal field of view in degrees, measured from where the moon and the tree fall in an H5 login
/// screenshot at a 1.9 aspect ratio.
// ponytail: fixed horizontal FOV; if other aspect ratios frame differently from H5, fix the vertical one instead.
const FOV: f32 = 50.0;
/// View-projection matrix, fog color, fog start and end with the near plane, and the hour's light; see `Globals`
/// in `scene.wgsl`.
const GLOBALS_BYTES: u64 = 96 + 13 * 16;

/// Geometry drawn with one material: a range of level indices or of particle quad indices.
struct Batch {
    material: Material,
    draw: Draw,
    fogged: bool,
    /// Units over which a soft sprite fades in front of the geometry behind it; zero when hard.
    soft: f32,
    uniform: wgpu::Buffer,
    /// One bind group a frame of the base texture, the still first one and then its `AnimNext` chain.
    groups: Vec<wgpu::BindGroup>,
    /// Frames of that chain a second; zero where the base does not animate.
    fps: f32,
    indices: Range<u32>,
    /// For a particle system's sprites, the zone they draw in.
    zone: Option<String>,
}

pub(crate) struct Scene {
    pipeline: Pipeline,
    globals: wgpu::Buffer,
    /// Globals for level geometry, with a placeholder where the depth being drawn cannot be read.
    globals_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    batches: Vec<Batch>,
    systems: Vec<System>,
    /// Swaying meshes, whose vertices follow the particles' in the particle buffers, and their batches, which draw
    /// with the level geometry since they write depth.
    movers: Vec<movers::Mover>,
    mover_batches: Vec<Batch>,
    /// One batch per particle system, in the same order.
    sprite_batches: Vec<Batch>,
    particle_vertices: wgpu::Buffer,
    particle_indices: wgpu::Buffer,
    pawns: Vec<Pawn>,
    /// One batch per pawn part section, into the pawn buffers the parts fill in order.
    pawn_batches: Vec<Batch>,
    pawn_vertices: wgpu::Buffer,
    pawn_indices: wgpu::Buffer,
    /// Where the map was loaded from: level, particle and pawn vertices are placed relative to it.
    camera: [f32; 3],
    /// Where the camera stands now.
    eye: Placement,
    /// Every scene's camera shots, by the scene's tag in lowercase, and the camera's move along some, if moving.
    shots: BTreeMap<String, Vec<Shot>>,
    flight: Option<flight::Flight>,
    /// How fast the player turns the pawns they may turn, in rotation units a second, and the scene time they last
    /// turned at.
    turning: (f32, f32),
    catalog: Catalog,
    /// The hour's light in world zones; in zones with states, everything draws with the light the level stored.
    daylight: Option<daylight::Daylight>,
    /// The scene the camera was last placed by, whose zone gives the fog and the particles drawn, and every scene's
    /// warp by its tag in lowercase.
    warp: Warp,
    warps: BTreeMap<String, Warp>,
    /// Zero of the clock materials and particles run on.
    started: Instant,
    /// Whether every batch's uniform holds its material; after that only animated materials are rewritten.
    uniforms_written: bool,
    /// Depth target, the size it was made for, the globals group particles read it through, and the
    /// multisampled color target resolved into the window's frame.
    depth: Option<(wgpu::TextureView, [u32; 2], wgpu::BindGroup, wgpu::TextureView)>,
}

impl Scene {
    /// Loads `map` from the client, framed by the scene tagged `camera_tag`.
    pub(crate) fn load(gpu: &Gpu, client_root: &Path, map: &str, camera_tag: &str) -> Result<Self, String> {
        let data = load::load(client_root, map, camera_tag)?;
        let (device, queue) = (&gpu.device, &gpu.queue);
        let mut pipeline = Pipeline::new(device, gpu.config.format.remove_srgb_suffix());
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene globals"),
            size: GLOBALS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_group = pipeline.globals_group(device, &globals, &depth_texture(device, [1, 1]));
        let views: HashMap<&str, wgpu::TextureView> = data
            .textures
            .iter()
            .map(|(path, image)| (path.as_str(), Pipeline::texture(device, queue, image)))
            .collect();
        let view = |path: &str| views.get(path);
        let mut batch = |material: Material, draw: Draw, fogged: bool, soft: f32, indices: Range<u32>| {
            material_batch(&mut pipeline, device, &view, material, draw, indices).map(|batch| Batch {
                fogged,
                soft,
                ..batch
            })
        };
        let batches: Vec<Batch> = data
            .batches
            .into_iter()
            .filter_map(|level| {
                batch(level.material.clone(), Draw::surface(level.material.blend), level.fogged, 0.0, level.indices)
            })
            .collect();

        let mut systems =
            particles::start(&data.emitters, data.camera.location, data.cloud_tint, &data.particle_meshes);
        systems.retain(|system| {
            system.mesh.is_some() || system.sprite.texture.as_deref().is_some_and(|path| views.contains_key(path))
        });
        let (sprite_batches, particle_vertex_count, mut particle_index_data) = emission::layout(&systems, &mut batch)?;
        let (mover_batches, mover_vertex_count) =
            movers::layout(&data.movers, particle_vertex_count, &mut particle_index_data, &mut batch)?;
        let particle_vertex_count = particle_vertex_count + mover_vertex_count;
        let quads: usize = systems.iter().map(System::len).sum();

        let (pawn_vertices, pawn_indices) = cast::pawn_buffers(device, &pawns::layout(&[]));

        let vertices = buffer(device, "scene vertices", wgpu::BufferUsages::VERTEX, &vertex_bytes(&data.vertices));
        let indices = buffer(device, "scene indices", wgpu::BufferUsages::INDEX, &index_bytes(&data.indices));
        let (particle_vertices, particle_indices) =
            particle_buffers(device, particle_vertex_count, &particle_index_data);
        println!(
            "scene: {map} from {camera_tag} at {:?} turned {:?}, {} vertices, {} triangles, {} materials, {} textures, {} particle systems with {quads} particles, {} swaying meshes",
            data.camera.location,
            data.camera.rotation,
            data.vertices.len(),
            data.indices.len() / 3,
            batches.len(),
            views.len(),
            systems.len(),
            data.movers.len(),
        );
        Ok(Self {
            pipeline,
            globals,
            globals_group,
            vertices,
            indices,
            batches,
            systems,
            movers: data.movers,
            mover_batches,
            sprite_batches,
            particle_vertices,
            particle_indices,
            pawns: Vec::new(),
            pawn_batches: Vec::new(),
            pawn_vertices,
            pawn_indices,
            camera: data.camera.location,
            eye: data.camera,
            shots: data.shots,
            flight: None,
            turning: (0.0, 0.0),
            catalog: data.catalog,
            daylight: data.daylight,
            warp: data.warp,
            warps: data.warps,
            started: Instant::now(),
            uniforms_written: false,
            depth: None,
        })
    }

    /// Cuts the camera to the scene tagged `tag`, as loading the map from it would place it; false when the map has
    /// no such scene or it lands in a zone lit another way, which needs the map loaded from it.
    pub(crate) fn warp(&mut self, tag: &str) -> bool {
        match self.warps.get(&tag.to_ascii_lowercase()) {
            Some(warp) if warp.zone_state == self.warp.zone_state => {
                (self.eye, self.flight) = (warp.placement, None);
                self.warp = warp.clone();
                true
            }
            _ => false,
        }
    }

    /// Turns the pawns the player may turn at `speed` rotation units a second from now on; 0 stops them.
    pub(crate) fn turn(&mut self, speed: f32) {
        self.turning.0 = speed;
    }

    /// Each pawn's label and where its head and feet fall on a window of `size` physical pixels, in those pixels;
    /// pawns behind the camera are left out.
    pub(crate) fn labels(&self, size: [u32; 2]) -> Vec<(&str, [f32; 2], [f32; 2])> {
        let [width, height] = size.map(|side| side as f32);
        // Columns of world x, y, z and w; rows clip x, clip y, depth and the distance along the view.
        let [[xx, xy, _, xw], [yx, yy, _, yw], [zx, zy, _, zw], [wx, wy, _, ww]] = self.view(width / height.max(1.0));
        let project = |[x, y, z]: [f32; 3]| {
            let distance = xw * x + yw * y + zw * z + ww;
            let clip_x = xx * x + yx * y + zx * z + wx;
            let clip_y = xy * x + yy * y + zy * z + wy;
            (distance > camera::NEAR)
                .then(|| [(clip_x / distance).midpoint(1.0) * width, (-clip_y / distance).midpoint(1.0) * height])
        };
        let time = self.started.elapsed().as_secs_f32();
        self.pawns
            .iter()
            .filter_map(|pawn| {
                let (label, head) = pawn.label(time, self.camera)?;
                Some((label, project(head)?, project(pawn.feet(self.camera))?))
            })
            .collect()
    }

    /// The label of the pawn at `point` on a window of `size` physical pixels, the nearest when several are.
    // ponytail: a box from the label down to the feet, a third as wide as tall; pick by mesh when it misses.
    pub(crate) fn label_at(&self, size: [u32; 2], [px, py]: [f32; 2]) -> Option<&str> {
        self.labels(size)
            .into_iter()
            .filter(|(_, [hx, hy], [_, fy])| {
                px > hx - (fy - hy) / 6.0 && px < hx + (fy - hy) / 6.0 && py > *hy && py < *fy
            })
            .min_by(|(_, [a, _], _), (_, [b, _], _)| (a - px).abs().total_cmp(&(b - px).abs()))
            .map(|(label, _, _)| label)
    }

    /// Clears the window's `frame` and draws the scene into it as it looks now.
    pub(crate) fn draw(&mut self, gpu: &Gpu, frame: &wgpu::Texture) -> wgpu::CommandBuffer {
        let size = gpu.size();
        let aspect = size[0] as f32 / size[1].max(1) as f32;
        self.fly();
        gpu.queue.write_buffer(&self.globals, 0, &self.globals_uniform(aspect));
        let time = self.started.elapsed().as_secs_f32();
        // Most materials never change; rewriting all their uniforms each frame cost Lobby02 over 50 ms.
        let batches =
            self.batches.iter().chain(&self.sprite_batches).chain(&self.pawn_batches).chain(&self.mover_batches);
        for batch in batches.filter(|batch| !self.uniforms_written || animated(&batch.material)) {
            gpu.queue.write_buffer(
                &batch.uniform,
                0,
                &material_uniform(&batch.material, time, batch.fogged, batch.soft),
            );
        }
        self.uniforms_written = true;
        for system in &mut self.systems {
            system.update(time);
        }
        let mut sprites = Vec::new();
        for system in &self.systems {
            system.write(time, self.eye.rotation, &mut sprites);
        }
        for mover in &self.movers {
            mover.write(time, self.camera, &mut sprites);
        }
        gpu.queue.write_buffer(&self.particle_vertices, 0, &vertex_bytes(&sprites));
        if !self.pawns.is_empty() {
            let (speed, last) = (self.turning.0, std::mem::replace(&mut self.turning.1, time));
            for pawn in &mut self.pawns {
                pawn.turn(speed, time - last);
            }
            let mut skinned = Vec::new();
            for pawn in &self.pawns {
                pawn.write(time, self.camera, self.daylight.is_some(), &mut skinned);
            }
            gpu.queue.write_buffer(&self.pawn_vertices, 0, &vertex_bytes(&skinned));
        }
        if self.depth.as_ref().is_none_or(|(_, depth_size, _, _)| *depth_size != size) {
            let view = depth_texture(&gpu.device, size);
            let format = gpu.config.format.remove_srgb_suffix();
            let color = target(&gpu.device, size, format, wgpu::TextureUsages::RENDER_ATTACHMENT);
            let group = self.pipeline.globals_group(&gpu.device, &self.globals, &view);
            self.depth = Some((view, size, group, color));
        }
        // Gamma-space colors written and blended as they are, as the original client's framebuffer did.
        let target = frame.create_view(&wgpu::TextureViewDescriptor {
            format: Some(gpu.config.format.remove_srgb_suffix()),
            ..Default::default()
        });
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("scene") });
        let Some((depth, _, particle_globals, color)) = &self.depth else { return encoder.finish() };
        // Level geometry first, writing depth; then particles over it in a pass that only reads depth, so
        // soft sprites can sample it.
        // ponytail: particle systems draw in level order, not sorted by distance; sort them if overlaps show.
        // Characters draw with the level geometry, from their own buffers.
        let level = [
            (&self.batches, &self.vertices, &self.indices),
            (&self.pawn_batches, &self.pawn_vertices, &self.pawn_indices),
            (&self.mover_batches, &self.particle_vertices, &self.particle_indices),
        ];
        let sprites = [(&self.sprite_batches, &self.particle_vertices, &self.particle_indices)];
        let passes: [(&[_], _, bool); 2] = [(&level, &self.globals_group, true), (&sprites, particle_globals, false)];
        for (sets, globals, first) in passes {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color,
                    depth_slice: None,
                    resolve_target: Some(&target),
                    ops: wgpu::Operations {
                        load: if first { wgpu::LoadOp::Clear(wgpu::Color::BLACK) } else { wgpu::LoadOp::Load },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: first
                        .then_some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, globals, &[]);
            for (batches, vertices, indices) in sets {
                pass.set_vertex_buffer(0, vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                // Other zones are closed off from the camera's; only its own emitters can be seen.
                // ponytail: zones stand in for BSP portal visibility; add portals when a scene looks into another zone.
                for batch in batches.iter().filter(|batch| batch.zone.is_none() || batch.zone == self.warp.zone) {
                    let Some(pipeline) = self.pipeline.get(batch.draw) else { continue };
                    pass.set_pipeline(pipeline);
                    let Some(group) = batch.group(time) else { continue };
                    pass.set_bind_group(1, group, &[]);
                    pass.draw_indexed(batch.indices.clone(), 0, 0..1);
                }
            }
        }
        encoder.finish()
    }
}

impl Scene {
    /// The globals the shader reads this frame: the view, the fog of the camera's zone and the hour's light.
    fn globals_uniform(&self, aspect: f32) -> Vec<u8> {
        let matrix = self.view(aspect);
        // Without fog, the range starts beyond any distance drawn.
        let (fog_color, fog_range) = self.warp.fog.map_or(([0.0; 4], [f32::MAX, f32::MAX, camera::NEAR, 0.0]), |fog| {
            (fog.color.map(|channel| f32::from(channel) / 255.0), [fog.start, fog.end, camera::NEAR, 0.0])
        });
        let light = self.daylight.as_ref().map_or([[0.0; 4]; 13], daylight::Daylight::uniform);
        matrix
            .iter()
            .chain([&fog_color, &fog_range])
            .chain(&light)
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }
}

impl Batch {
    /// The bind group of the frame showing at `time`, which stands still where the base does not animate.
    fn group(&self, time: f32) -> Option<&wgpu::BindGroup> {
        self.groups.get(frame_at(time, self.fps, self.groups.len())).or_else(|| self.groups.first())
    }
}

/// Which of `count` frames a chain playing `fps` frames a second shows at `time` seconds.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a frame number, wrapped to the count")]
fn frame_at(time: f32, fps: f32, count: usize) -> usize {
    (time.max(0.0) * fps) as usize % count.max(1)
}

/// Whether `material`'s uniform changes with time: its stages pan or rotate, or its tint fades.
fn animated(material: &Material) -> bool {
    material.fade.is_some()
        || material.glow.is_some()
        || std::iter::once(&material.base)
            .chain(material.layer.as_ref().map(|(stage, _, _)| stage))
            .flat_map(|stage| &stage.uv)
            .any(|modifier| matches!(modifier, UvModifier::Pan(_) | UvModifier::Rotate { .. }))
}

/// The batch that draws `indices` with `material`, fogged and hard-edged, when its textures have views.
fn material_batch<'a>(
    pipeline: &mut Pipeline,
    device: &wgpu::Device,
    view: &dyn Fn(&str) -> Option<&'a wgpu::TextureView>,
    material: Material,
    draw: Draw,
    indices: Range<u32>,
) -> Option<Batch> {
    let base = view(&material.base.texture)?;
    // A material without a second stage samples its base twice; the shader ignores it.
    let layer = material.layer.as_ref().and_then(|(stage, _, _)| view(&stage.texture));
    let uniform = material_buffer(device);
    // Every frame of an animated base gets a group of its own, over the one uniform they share.
    let frames = material.frames.iter().filter_map(|frame| view(frame));
    let groups = std::iter::once(base)
        .chain(frames)
        .map(|frame| pipeline.material(device, &uniform, frame, layer.unwrap_or(frame)))
        .collect();
    pipeline.prepare(device, draw);
    Some(Batch { fps: material.fps, material, draw, fogged: true, soft: 0.0, uniform, groups, indices, zone: None })
}

/// A vertex buffer of `vertices` the particles rewrite every frame, and their fixed `indices`; neither empty.
fn particle_buffers(device: &wgpu::Device, vertices: usize, indices: &[u32]) -> (wgpu::Buffer, wgpu::Buffer) {
    let buffer_of_vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particle vertices"),
        size: (vertices.max(1) * size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let indices = if indices.is_empty() { &[0; 3][..] } else { indices };
    (buffer_of_vertices, buffer(device, "particle indices", wgpu::BufferUsages::INDEX, &index_bytes(indices)))
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

#[cfg(test)]
mod tests {
    use super::frame_at;

    #[test]
    fn an_animated_material_walks_its_frames_and_starts_over() {
        assert_eq!(frame_at(0.0, 25.0, 16), 0);
        assert_eq!(frame_at(0.04, 25.0, 16), 1, "a frame every 1/25 of a second");
        assert_eq!(frame_at(0.64, 25.0, 16), 0, "sixteen frames later it is back at the first");
        assert_eq!(frame_at(9.0, 0.0, 1), 0, "a material that does not animate keeps its one frame");
    }
}

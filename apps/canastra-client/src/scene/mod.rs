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
mod ground;
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
mod world;

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::path::Path;
use std::time::Instant;

use l2_catalog::{Catalog, Material, UvModifier};
use ue2_level::{AmbientSound, Heard, Placement, Shot, Warp};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::audio::Clip;
use crate::gpu::Gpu;
pub(crate) use flight::Route;
pub(crate) use load::SceneData;
use load::Vertex;
pub(crate) use load::View;
use particles::System;
use pawns::Pawn;
pub(crate) use pawns::{Figure, HeldSource, PartSource};
use pipeline::{Draw, Pipeline, depth_texture, material_buffer, material_uniform, target};
pub(crate) use world::map_at;

/// Horizontal field of view in degrees, measured from where the moon and the tree fall in an H5 login
/// screenshot at a 1.9 aspect ratio.
// ponytail: fixed horizontal FOV; if other aspect ratios frame differently from H5, fix the vertical one instead.
const FOV: f32 = 50.0;
/// How far above a floor the camera keeps itself, in world units.
const CLEARANCE: f32 = 20.0;
/// How far a click reaches into the world looking for the floor, in world units.
// ponytail: as far as a tile's quarter; the fog of world zones is unread, and it is what H5 stops drawing at.
const REACH: f32 = 8192.0;
/// What a scene turned out to hold, for the log.
#[derive(Clone, Copy)]
struct Held {
    batches: usize,
    textures: usize,
    systems: usize,
    quads: usize,
}

fn report(map: &str, camera: Placement, vertices: usize, indices: usize, held: Held) {
    let Held { batches, textures, systems, quads } = held;
    println!(
        "scene: {map} at {:?} turned {:?}, {vertices} vertices, {} triangles, {batches} batches, {textures} textures, {systems} particle systems with {quads} particles",
        camera.location,
        camera.rotation,
        indices / 3,
    );
}

/// The buffer the shader reads the view, the fog and the hour's light from.
fn globals_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scene globals"),
        size: GLOBALS_BYTES,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// How many materials a scene keeps side by side: one for every batch that draws in it, the level's own
/// chunks, the particle systems and the swaying meshes.
fn slots_of(data: &SceneData, systems: &[System]) -> usize {
    let sprites: usize = systems.iter().map(|system| system.mesh.as_ref().map_or(1, |mesh| mesh.sections.len())).sum();
    let movers: usize = data.movers.iter().map(movers::Mover::sections).sum();
    data.batches.len() + sprites + movers
}

/// Where a batch's material is kept: the buffer it shares and its place in it.
#[derive(Clone, Copy)]
pub(super) struct Slot<'a> {
    pub(super) uniforms: &'a wgpu::Buffer,
    pub(super) slot: usize,
}

/// Bytes a batch's material takes in the buffer its scene shares.
const SLOT_BYTES: usize = 256;
/// How far from the camera a particle system still shows, in world units.
// ponytail: one radius for every system; the client's own emitters carry no reach of their own.
const PARTICLE_REACH: f32 = 4000.0;
/// How many ambient sounds play at once: the loudest where the camera stands.
const VOICES: usize = 8;
/// The hours between which a scene counts as daylight, from where the client's light ramps turn warm at dawn to
/// where they turn warm again at dusk: birds and cicadas are heard between them, crickets and wolves outside.
const DAWN: f32 = 6.0;
const DUSK: f32 = 21.0;
/// View-projection matrix, fog color, fog start and end with the near plane, and the hour's light; see `Globals`
/// in `scene.wgsl`.
const GLOBALS_BYTES: u64 = 96 + 16 * 16;

/// Geometry drawn with one material: a range of level indices or of particle quad indices.
struct Batch {
    material: Material,
    draw: Draw,
    fogged: bool,
    /// Units over which a soft sprite fades in front of the geometry behind it; zero when hard.
    soft: f32,
    /// Where this batch's material sits in the buffer its scene shares.
    slot: usize,
    /// One bind group a frame of the base texture, the still first one and then its `AnimNext` chain.
    groups: Vec<wgpu::BindGroup>,
    /// Frames of that chain a second; zero where the base does not animate.
    fps: f32,
    indices: Range<u32>,
    /// For a particle system's sprites, the zone they draw in.
    zone: Option<String>,
    /// For a particle system's sprites, which system they belong to, so far ones are left out.
    system: Option<usize>,
    /// The box this batch fills, for leaving out what falls outside the screen; what moves with the camera,
    /// such as the sky and the characters, has none.
    bounds: Option<[[f32; 3]; 2]>,
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
    /// The sounds the level plays around places, and the hour they are heard at.
    ambient_sounds: Vec<AmbientSound>,
    hour: f32,
    /// Swaying meshes, whose vertices follow the particles' in the particle buffers, and their batches, which draw
    /// with the level geometry since they write depth.
    movers: Vec<movers::Mover>,
    mover_batches: Vec<Batch>,
    /// One batch per particle system, in the same order.
    sprite_batches: Vec<Batch>,
    /// Where each system's vertices begin in the particle buffer, and where the swaying meshes begin.
    system_starts: Vec<usize>,
    mover_start: usize,
    /// Which systems are close enough to the camera to draw, worked out once a frame.
    near: Vec<bool>,
    /// Which way the camera looks, which is its own and not the character's: a character turns where it
    /// walks, and the view behind it stays where it was left.
    watching: Option<i32>,
    /// Every particle and swaying mesh vertex, as the scene last wrote them. A frame fills in what moved and
    /// sends the stretch that covers it in one go: a write costs far more than the bytes it moves.
    sprites: Vec<Vertex>,
    /// Vertices written each frame, kept between frames so no frame allocates them again.
    scratch: Vec<Vertex>,
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
    /// The floors of the map, which characters stand on and clicks land on.
    ground: ground::Ground,
    /// Every batch's material, side by side in one buffer, with the copy a frame fills in and writes at once.
    materials: wgpu::Buffer,
    mirror: Vec<u8>,
    pawn_materials: wgpu::Buffer,
    pawn_mirror: Vec<u8>,
    /// Where each character in the scene stands, which way it faces and whether it is on its way, in the
    /// order the figures were placed.
    steering: Vec<([f32; 3], i32, bool)>,
    /// The hour's light in world zones; in zones with states, everything draws with the light the level stored.
    daylight: Option<daylight::Daylight>,
    /// In world zones, the client's time-of-day ramps and sun path, which the hour follows.
    environment: Option<l2_env::Environment>,
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
    /// Reads `map` from the client, framed as `view` says. This is the slow half, which touches no GPU and
    /// runs off the window's thread; [`Scene::build`] makes a scene of what it read.
    pub(crate) fn read(client_root: &Path, map: &str, view: View<'_>, blocks: bool) -> Result<SceneData, String> {
        load::load(client_root, map, view, blocks)
    }

    /// Builds the scene of what [`Scene::read`] read of `map`.
    pub(crate) fn build(gpu: &Gpu, map: &str, mut data: SceneData) -> Result<Self, String> {
        let (device, queue) = (&gpu.device, &gpu.queue);
        let mut pipeline = Pipeline::new(device, gpu.config.format.remove_srgb_suffix());
        let globals = globals_buffer(device);
        let globals_group = pipeline.globals_group(device, &globals, &depth_texture(device, [1, 1]));
        let views: HashMap<&str, wgpu::TextureView> = data
            .textures
            .iter()
            .map(|(path, image)| (path.as_str(), Pipeline::texture(device, queue, image)))
            .collect();
        let view = |path: &str| views.get(path);
        let mut systems =
            particles::start(&data.emitters, data.camera.location, data.cloud_tint, &data.particle_meshes);
        systems.retain(|system| {
            system.mesh.is_some() || system.sprite.texture.as_deref().is_some_and(|path| views.contains_key(path))
        });
        systems.sort_by_key(System::cell);
        let slots = slots_of(&data, &systems);
        let materials = material_buffer(device, slots);
        let mut slot = 0;
        let mut batch = |material: Material, draw: Draw, fogged: bool, soft: f32, indices: Range<u32>| {
            let made = material_batch(
                &mut pipeline,
                device,
                &view,
                material,
                draw,
                indices,
                Slot { uniforms: &materials, slot },
            );
            slot += 1;
            made.map(|batch| Batch { fogged, soft, ..batch })
        };
        let batches = level_batches(data.batches, &mut batch);
        let laid = emission::layout(&systems, &mut batch)?;
        let (sprite_batches, system_starts) = (laid.batches, laid.starts);
        let (particle_vertex_count, mut particle_index_data) = (laid.vertices, laid.indices);
        let (mover_batches, mover_vertex_count) =
            movers::layout(&data.movers, particle_vertex_count, &mut particle_index_data, &mut batch)?;
        let particle_vertex_count = particle_vertex_count + mover_vertex_count;
        let quads: usize = systems.iter().map(System::len).sum();
        // The packages the map was built from are let go once it is on the GPU: a tile's worth of them is
        // hundreds of megabytes, and what a character needs later is read again then.
        data.catalog.forget();

        let (pawn_vertices, pawn_indices) = cast::pawn_buffers(device, &pawns::layout(&[]));

        let vertices = buffer(device, "scene vertices", wgpu::BufferUsages::VERTEX, &vertex_bytes(&data.vertices));
        let indices = buffer(device, "scene indices", wgpu::BufferUsages::INDEX, &index_bytes(&data.indices));
        let (particle_vertices, particle_indices) =
            particle_buffers(device, particle_vertex_count, &particle_index_data);
        let held = Held { batches: batches.len(), textures: views.len(), systems: systems.len(), quads };
        report(map, data.camera, data.vertices.len(), data.indices.len(), held);
        Ok(Self {
            pipeline,
            globals,
            globals_group,
            vertices,
            indices,
            batches,
            systems,
            ambient_sounds: data.ambient_sounds,
            hour: data.hour,
            movers: data.movers,
            mover_batches,
            sprite_batches,
            system_starts,
            mover_start: particle_vertex_count - mover_vertex_count,
            near: Vec::new(),
            watching: None,
            sprites: vec![[0.0; 15]; particle_vertex_count],
            scratch: Vec::new(),
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
            ground: data.ground,
            materials,
            mirror: vec![0; slots.max(1) * SLOT_BYTES],
            pawn_materials: material_buffer(device, 1),
            pawn_mirror: Vec::new(),
            steering: Vec::new(),
            daylight: data.daylight,
            environment: data.environment,
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

    /// What is heard where the camera stands: each sound's file, how loudly it plays, how often it is called and
    /// how fast; loudest first and at most `VOICES` of them. Sounds of the day are left out at night and the other
    /// way round.
    // ponytail: sounds fade linearly to nothing at their radius, and none of them pan; measure against H5 if the
    // lobby sounds wrong.
    pub(crate) fn ambient_sounds(&mut self) -> Vec<Clip> {
        let eye = self.eye.location;
        let daylight = (DAWN..DUSK).contains(&self.hour);
        let mut heard: Vec<(AmbientSound, f32)> = self
            .ambient_sounds
            .iter()
            .filter(|sound| match sound.heard {
                Heard::Day => daylight,
                Heard::Night => !daylight,
                Heard::Always => true,
            })
            .filter_map(|sound| {
                let distance = sound.location.iter().zip(eye).map(|(at, eye)| (at - eye) * (at - eye)).sum::<f32>();
                let reach = 1.0 - distance.sqrt() / sound.radius.max(1.0);
                (reach > 0.0).then(|| (sound.clone(), sound.volume * reach))
            })
            .collect();
        heard.sort_by(|a, b| b.1.total_cmp(&a.1));
        heard.truncate(VOICES);
        heard
            .into_iter()
            .filter_map(|(sound, gain)| {
                let bytes = self.catalog.sound(&sound.sound)?;
                Some(Clip { bytes, gain, interval: sound.interval, pitch: sound.pitch })
            })
            .collect()
    }

    /// Puts each character in the scene where it now stands, on the floor under it, and moves the camera
    /// behind the one at `player`, whose body reaches `middle` above its feet.
    pub(crate) fn steer(&mut self, steps: &[([f32; 3], i32, bool)], player: usize, middle: f32) {
        self.steering = steps.iter().map(|(at, yaw, moving)| (self.on_ground(*at), *yaw, *moving)).collect();
        let Some(&(at, yaw, _)) = self.steering.get(player) else { return };
        // The camera looks the way it was left; only a character that has just arrived brings it its own.
        let watching = *self.watching.get_or_insert(yaw);
        self.eye = world::behind(at, watching, middle);
        // The camera keeps clear of the floor it would stand in, such as the rise of a terrace behind a
        // character on it.
        // ponytail: it is lifted where H5 would instead pull it in toward the character.
        let [x, y, z] = self.eye.location;
        let [ox, oy, oz] = self.camera;
        // Only ground no higher than a step above the character counts, so a roof over it never lifts it.
        if let Some(floor) = self.ground.under([x - ox, y - oy, at[2] - oz]) {
            let clear = floor + oz + CLEARANCE;
            if clear > z {
                self.eye.location = [x, y, clear];
            }
        }
    }

    /// Where a place stands on the floor under it, or where it is when the map has no floor there.
    pub(crate) fn on_ground(&self, at: [f32; 3]) -> [f32; 3] {
        let [x, y, z] = at;
        let [ox, oy, oz] = self.camera;
        match self.ground.under([x - ox, y - oy, z - oz]) {
            Some(height) => [x, y, height + oz],
            None => at,
        }
    }

    /// Where the floor is under the point `pixel` of a window of `size` physical pixels, for a click that asks
    /// the character to walk there; `None` where the ray meets no floor.
    pub(crate) fn ground_at(&self, size: [u32; 2], [px, py]: [f32; 2]) -> Option<[f32; 3]> {
        let [width, height] = size.map(|side| side as f32);
        let aspect = width / height.max(1.0);
        let [forward, right, up] = camera::axes(self.eye.rotation);
        let across = 1.0 / (FOV.to_radians() / 2.0).tan();
        let (x, y) = (px / width * 2.0 - 1.0, 1.0 - py / height * 2.0);
        let (sideways, upward) = (x / across, y / (across * aspect));
        let mut direction = forward;
        for ((direction, right), up) in direction.iter_mut().zip(right).zip(up) {
            *direction += right * sideways + up * upward;
        }
        let length = direction.iter().map(|value| value * value).sum::<f32>().sqrt();
        let direction = direction.map(|value| value / length.max(f32::EPSILON));
        let mut eye = self.eye.location;
        for (eye, origin) in eye.iter_mut().zip(self.camera) {
            *eye -= origin;
        }
        let mut hit = self.ground.hit(eye, direction, REACH)?;
        for (hit, origin) in hit.iter_mut().zip(self.camera) {
            *hit += origin;
        }
        Some(hit)
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
        self.follow_clock();
        gpu.queue.write_buffer(&self.globals, 0, &self.globals_uniform(aspect));
        let time = self.started.elapsed().as_secs_f32();
        // Materials are kept side by side in one buffer, so a frame that changes a few of them still costs
        // a single write. Most never change: only the ones that move are filled in again.
        let scene = self.batches.iter().chain(&self.sprite_batches).chain(&self.mover_batches);
        let fill = |mirror: &mut Vec<u8>, batch: &Batch| {
            let bytes = material_uniform(&batch.material, time, batch.fogged, batch.soft);
            let at = usize::try_from(pipeline::slot_at(batch.slot)).unwrap_or(0);
            if let Some(slot) = mirror.get_mut(at..at + bytes.len()) {
                slot.copy_from_slice(&bytes);
            }
        };
        let mut mirror = std::mem::take(&mut self.mirror);
        for batch in scene.filter(|batch| !self.uniforms_written || animated(&batch.material)) {
            fill(&mut mirror, batch);
        }
        gpu.queue.write_buffer(&self.materials, 0, &mirror);
        self.mirror = mirror;
        let mut pawn_mirror = std::mem::take(&mut self.pawn_mirror);
        for batch in self.pawn_batches.iter().filter(|batch| !self.uniforms_written || animated(&batch.material)) {
            fill(&mut pawn_mirror, batch);
        }
        if !pawn_mirror.is_empty() {
            gpu.queue.write_buffer(&self.pawn_materials, 0, &pawn_mirror);
        }
        self.pawn_mirror = pawn_mirror;
        self.uniforms_written = true;
        self.write_particles(gpu, time);
        self.write_pawns(gpu, time);
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
        // What the camera cannot see is not drawn: a map holds far more than a screen shows, and what lies
        // beyond the fog is already hidden by it.
        let screen = camera::sides(&self.view(aspect));
        let [eye_x, eye_y, eye_z] = self.eye.location;
        let [origin_x, origin_y, origin_z] = self.camera;
        let eye = [eye_x - origin_x, eye_y - origin_y, eye_z - origin_z];
        let reach = self.warp.fog.map_or(f32::MAX, |fog| fog.end);
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
                let shown = |batch: &&Batch| {
                    let zone = batch.zone.is_none() || batch.zone == self.warp.zone;
                    let near = batch.system.is_none_or(|system| self.near.get(system).copied().unwrap_or(false));
                    let seen = batch.bounds.is_none_or(|bounds| {
                        camera::in_view(&screen, bounds) && camera::within_reach(bounds, eye, reach)
                    });
                    zone && near && seen
                };
                for batch in batches.iter().filter(shown) {
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
    /// Moves every particle near the camera and writes what it draws. Systems too far to be seen are neither
    /// moved nor written, and their old vertices are never read, since their batches do not draw either.
    fn write_particles(&mut self, gpu: &Gpu, time: f32) {
        let [eye_x, eye_y, eye_z] = self.eye.location;
        let [origin_x, origin_y, origin_z] = self.camera;
        let eye = [eye_x - origin_x, eye_y - origin_y, eye_z - origin_z];
        self.near = self.systems.iter().map(|system| system.within(eye, PARTICLE_REACH)).collect();
        for (system, near) in self.systems.iter_mut().zip(&self.near) {
            if *near {
                system.update(time);
            }
        }
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut sprites = std::mem::take(&mut self.sprites);
        let (mut first, mut last) = (usize::MAX, 0usize);
        let mut fill = |at: usize, written: &[Vertex]| {
            if let Some(slice) = sprites.get_mut(at..at + written.len()) {
                slice.copy_from_slice(written);
                (first, last) = (first.min(at), last.max(at + written.len()));
            }
        };
        for (index, system) in self.systems.iter().enumerate() {
            if !self.near.get(index).copied().unwrap_or(false) {
                continue;
            }
            scratch.clear();
            system.write(time, self.eye.rotation, &mut scratch);
            fill(self.system_starts.get(index).copied().unwrap_or(0), &scratch);
        }
        if !self.movers.is_empty() {
            scratch.clear();
            for mover in &self.movers {
                mover.write(time, self.camera, &mut scratch);
            }
            fill(self.mover_start, &scratch);
        }
        if first < last {
            let moved = sprites.get(first..last).unwrap_or_default();
            gpu.queue.write_buffer(&self.particle_vertices, vertex_offset(first), &vertex_bytes(moved));
        }
        (self.scratch, self.sprites) = (scratch, sprites);
    }

    /// Moves every character in the scene and writes the skin they draw with.
    fn write_pawns(&mut self, gpu: &Gpu, time: f32) {
        if self.pawns.is_empty() {
            return;
        }
        let (speed, last) = (self.turning.0, std::mem::replace(&mut self.turning.1, time));
        for (index, pawn) in self.pawns.iter_mut().enumerate() {
            // In the world every character walks where the world says; in the lobby the player only turns the
            // one in front of them.
            match self.steering.get(index) {
                Some(&(at, yaw, moving)) => pawn.stride(at, yaw, moving, time - last),
                None => pawn.turn(speed, time - last),
            }
        }
        let mut skinned = Vec::new();
        for pawn in &self.pawns {
            pawn.write(time, self.camera, self.daylight.is_some(), &mut skinned);
        }
        gpu.queue.write_buffer(&self.pawn_vertices, 0, &vertex_bytes(&skinned));
    }

    /// Moves the light on to the lobby clock's hour once a game minute has passed.
    fn follow_clock(&mut self) {
        let Some(environment) = &self.environment else { return };
        let hour = environment.hour_after(daylight::clock_seconds());
        self.hour = hour;
        if self.daylight.as_ref().is_none_or(|daylight| (daylight.hour() - hour).abs() >= 1.0 / 60.0) {
            self.daylight = daylight::Daylight::new(environment, hour);
        }
    }

    /// The globals the shader reads this frame: the view, the fog of the camera's zone and the hour's light.
    fn globals_uniform(&self, aspect: f32) -> Vec<u8> {
        let matrix = self.view(aspect);
        // Without fog, the range starts beyond any distance drawn.
        let (fog_color, fog_range) = self.warp.fog.map_or(([0.0; 4], [f32::MAX, f32::MAX, camera::NEAR, 0.0]), |fog| {
            (fog.color.map(|channel| f32::from(channel) / 255.0), [fog.start, fog.end, camera::NEAR, 0.0])
        });
        let light = self.daylight.as_ref().map_or([[0.0; 4]; 16], daylight::Daylight::uniform);
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
/// The batches the level's own geometry draws with, each keeping the box it fills.
fn level_batches(
    level: Vec<load::Batch>,
    batch: &mut dyn FnMut(Material, Draw, bool, f32, Range<u32>) -> Option<Batch>,
) -> Vec<Batch> {
    level
        .into_iter()
        .filter_map(|chunk| {
            let draw = Draw::surface(chunk.material.blend);
            let made = batch(chunk.material, draw, chunk.fogged, 0.0, chunk.indices);
            made.map(|made| Batch { bounds: chunk.bounds, ..made })
        })
        .collect()
}

/// Where a vertex sits in the particle buffer, in bytes.
fn vertex_offset(vertex: usize) -> u64 {
    (vertex * size_of::<Vertex>()) as u64
}

fn material_batch<'a>(
    pipeline: &mut Pipeline,
    device: &wgpu::Device,
    view: &dyn Fn(&str) -> Option<&'a wgpu::TextureView>,
    material: Material,
    draw: Draw,
    indices: Range<u32>,
    slot: Slot<'_>,
) -> Option<Batch> {
    let Slot { uniforms, slot } = slot;
    let base = view(&material.base.texture)?;
    // A material without a second stage samples its base twice; the shader ignores it.
    let layer = material.layer.as_ref().and_then(|(stage, _, _)| view(&stage.texture));
    // Every frame of an animated base gets a group of its own, over the one uniform they share.
    let frames = material.frames.iter().filter_map(|frame| view(frame));
    let groups = std::iter::once(base)
        .chain(frames)
        .map(|frame| pipeline.material(device, uniforms, slot, frame, layer.unwrap_or(frame)))
        .collect();
    pipeline.prepare(device, draw);
    Some(Batch {
        fps: material.fps,
        material,
        draw,
        fogged: true,
        soft: 0.0,
        slot,
        groups,
        indices,
        zone: None,
        system: None,
        bounds: None,
    })
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

//! Sprite particles simulated on the CPU the way Unreal Engine 2's sprite emitters behave: spawned in a
//! box around the emitter, moving with velocity and acceleration, revolving about the emitter, sized
//! and colored by curves over their life, fading in and out, spinning, and respawning when they die. Mesh
//! emitters' particles move the same way and draw a mesh instead of a sprite.

use std::collections::HashMap;
use std::rc::Rc;

use ue2_level::{DrawStyle, Emitter, SpriteEmitter};

use super::camera;
use super::curves::{color_at, fade, size_at};
use super::load::Vertex;
use super::particle_mesh::{self, MeshKey, ParticleMesh};
use super::random::Random;

pub(crate) struct System {
    pub(crate) sprite: SpriteEmitter,
    /// The owning emitter's location relative to the camera.
    origin: [f32; 3],
    particles: Vec<Particle>,
    random: Random,
    /// RGB multiplier: the sky's color for sprites that use it, white otherwise.
    tint: [f32; 3],
    /// The zone the owning emitter stands in.
    pub(crate) zone: Option<String>,
    /// For a mesh emitter, the mesh each particle draws.
    pub(crate) mesh: Option<Rc<ParticleMesh>>,
    /// How the owning emitter is turned, which turns its meshes.
    axes: [[f32; 3]; 3],
}

struct Particle {
    /// Scene time of birth; negative for particles alive when the scene starts.
    born: f32,
    lifetime: f32,
    start: [f32; 3],
    velocity: [f32; 3],
    size: f32,
    /// A mesh particle's scale on each axis.
    scale: [f32; 3],
    /// Starting turn and turns per second.
    spin: [f32; 2],
    color: [f32; 3],
    /// Center relative to the emitter and turns per second about X, Y and Z.
    revolution: ([f32; 3], [f32; 3]),
    /// Texture cell, row by row.
    cell: u32,
}

/// One system per sprite emitter, started as if it had been running a whole lifetime, as Unreal's
/// warmup does, farthest first so blended systems draw back to front from the fixed camera. Sprites that
/// use the cloud color take `cloud_tint`; mesh emitters draw from `meshes` and are left out without their mesh.
pub(crate) fn start(
    emitters: &[Emitter],
    camera: [f32; 3],
    cloud_tint: [f32; 3],
    meshes: &HashMap<MeshKey, Rc<ParticleMesh>>,
) -> Vec<System> {
    let mut seed = 0x9E37_79B9_7F4A_7C15;
    let mut systems: Vec<System> = emitters
        .iter()
        .flat_map(|emitter| emitter.sprites.iter().map(move |sprite| (emitter, sprite)))
        .filter_map(|(emitter, sprite)| match &sprite.mesh {
            Some(shape) => Some((emitter, sprite, Some(meshes.get(&particle_mesh::key(shape))?.clone()))),
            None => sprite.texture.is_some().then_some((emitter, sprite, None)),
        })
        .map(|(emitter, sprite, mesh)| {
            let location = emitter.location;
            seed += 1;
            let mut random = Random(seed);
            let origin = [location[0] - camera[0], location[1] - camera[1], location[2] - camera[2]];
            let particles = (0..sprite.max_particles)
                .map(|_| {
                    let lifetime = random.range(sprite.lifetime).max(0.01);
                    let born = -random.unit() * lifetime;
                    spawn(sprite, &mut random, born, lifetime)
                })
                .collect();
            let tint = if sprite.cloud_color { cloud_tint } else { [1.0; 3] };
            System {
                sprite: sprite.clone(),
                origin,
                particles,
                random,
                tint,
                zone: emitter.zone.clone(),
                mesh,
                axes: camera::axes(emitter.rotation),
            }
        })
        .collect();
    let distance = |system: &System| {
        system
            .origin
            .iter()
            .zip(system.sprite.start_offset)
            .map(|(origin, offset)| (origin + offset).powi(2))
            .sum::<f32>()
    };
    systems.sort_by(|a, b| distance(b).total_cmp(&distance(a)));
    systems
}

impl System {
    /// Replaces particles whose life ended before `time`.
    pub(crate) fn update(&mut self, time: f32) {
        let Self { sprite, particles, random, .. } = self;
        for particle in particles.iter_mut() {
            if time - particle.born >= particle.lifetime {
                let lifetime = random.range(sprite.lifetime).max(0.01);
                *particle = spawn(sprite, random, particle.born + particle.lifetime, lifetime);
            }
        }
    }

    /// Each particle at `time`: its mesh, or four corners facing the camera given by its `rotation` unless the
    /// emitter lays sprites in a plane.
    pub(crate) fn write(&self, time: f32, rotation: [i32; 3], vertices: &mut Vec<Vertex>) {
        let [_, camera_right, camera_up] = camera::axes(rotation);
        let (right, up) = self.sprite.projection_normal.and_then(plane_axes).unwrap_or((camera_right, camera_up));
        let [columns, rows] = self.sprite.subdivisions;
        for particle in &self.particles {
            let age = (time - particle.born).max(0.0);
            let life = (age / particle.lifetime).clamp(0.0, 1.0);
            let center = self.origin_plus(particle, age);
            let size = particle.size * size_at(&self.sprite, life);
            let mut color = color_at(&self.sprite, life);
            for ((channel, multiplier), tint) in color.iter_mut().zip(particle.color).zip(self.tint) {
                *channel *= multiplier * tint;
            }
            color[3] *= self.sprite.opacity * fade(&self.sprite, age, particle.lifetime);
            if let Some(mesh) = &self.mesh {
                let grown = size_at(&self.sprite, life);
                mesh.write(center, particle.scale.map(|scale| scale * grown), &self.axes, color, vertices);
                continue;
            }
            let (sin, cos) = ((particle.spin[0] + particle.spin[1] * age) * std::f32::consts::TAU).sin_cos();
            let spun_right = mix(right, up, cos, sin);
            let spun_up = mix(up, right, cos, -sin);
            let (column, row) = (particle.cell % columns, (particle.cell / columns) % rows);
            let (u0, v0) = (column as f32 / columns as f32, row as f32 / rows as f32);
            let (u1, v1) = (u0 + 1.0 / columns as f32, v0 + 1.0 / rows as f32);
            for (horizontal, vertical, u, v) in
                [(-1.0, 1.0, u0, v0), (1.0, 1.0, u1, v0), (1.0, -1.0, u1, v1), (-1.0, -1.0, u0, v1)]
            {
                let corner = [0, 1, 2].map(|axis| {
                    center.get(axis).copied().unwrap_or_default()
                        + (spun_right.get(axis).copied().unwrap_or_default() * horizontal
                            + spun_up.get(axis).copied().unwrap_or_default() * vertical)
                            * size
                });
                vertices.push([corner[0], corner[1], corner[2], u, v, color[0], color[1], color[2], color[3]]);
            }
        }
    }

    fn origin_plus(&self, particle: &Particle, age: f32) -> [f32; 3] {
        let (center, turns) = particle.revolution;
        let mut offset = [0.0; 3];
        for ((((offset, start), velocity), acceleration), center) in
            offset.iter_mut().zip(particle.start).zip(particle.velocity).zip(self.sprite.acceleration).zip(center)
        {
            *offset = start + velocity * age + 0.5 * acceleration * age * age - center;
        }
        let offset = revolve(offset, turns, age);
        let mut at = self.origin;
        for ((at, offset), center) in at.iter_mut().zip(offset).zip(center) {
            *at += center + offset;
        }
        at
    }

    /// Particles this system draws.
    pub(crate) fn len(&self) -> usize {
        self.particles.len()
    }

    /// Vertices `write` writes for each particle.
    pub(crate) fn vertices_each(&self) -> usize {
        self.mesh.as_ref().map_or(4, |mesh| mesh.len())
    }
}

/// `offset` turned about the origin by `turns` per second on X, then Y, then Z, after `age` seconds.
// ponytail: the whole path turns at once; Unreal turns the location a tick at a time, which differs only for
// particles that also move.
fn revolve(mut offset: [f32; 3], turns: [f32; 3], age: f32) -> [f32; 3] {
    for (axis, turns) in turns.iter().enumerate() {
        let (sin, cos) = (turns * age * std::f32::consts::TAU).sin_cos();
        let [a, b] = [(axis + 1) % 3, (axis + 2) % 3];
        let (first, second) = (offset.get(a).copied().unwrap_or(0.0), offset.get(b).copied().unwrap_or(0.0));
        if let Some(value) = offset.get_mut(a) {
            *value = first * cos - second * sin;
        }
        if let Some(value) = offset.get_mut(b) {
            *value = first * sin + second * cos;
        }
    }
    offset
}

#[expect(clippy::cast_sign_loss, reason = "the random unit is never negative")]
fn spawn(sprite: &SpriteEmitter, random: &mut Random, born: f32, lifetime: f32) -> Particle {
    let mut start = sprite.start_offset;
    for (start, range) in start.iter_mut().zip(sprite.start_location) {
        *start += random.range(range);
    }
    let cells = sprite.subdivisions[0] * sprite.subdivisions[1];
    let [first, end] = sprite.subdivision_range;
    let end = if end == 0 { cells } else { end.min(cells) };
    let span = end.saturating_sub(first).max(1);
    Particle {
        born,
        lifetime,
        start,
        velocity: sprite.start_velocity.map(|range| random.range(range)),
        size: random.range(sprite.start_size),
        scale: sprite.mesh.as_ref().map_or([1.0; 3], |shape| shape.scale.map(|range| random.range(range))),
        spin: sprite.spin.map_or([0.0; 2], |(start, rate)| [random.range(start), random.range(rate)]),
        color: sprite.color_multiplier.map(|range| random.range(range)),
        revolution: sprite.revolution.map_or(([0.0; 3], [0.0; 3]), |(center, turns)| {
            (center.map(|range| random.range(range)), turns.map(|range| random.range(range)))
        }),
        cell: first + (random.unit() * span as f32) as u32 % span,
    }
}

/// How the emitter's draw style blends.
pub(crate) fn blend(style: DrawStyle) -> l2_catalog::Blend {
    use l2_catalog::Blend;
    match style {
        DrawStyle::Regular => Blend::Opaque,
        // ponytail: measured, not recovered. Against the H5 login, alpha-blended sprites brighten the bright
        // horizon and moon to H5's (253 vs 254 red) and only lift the dark hills, which plain alpha blending
        // cannot do; revisit if the client's blend state for PTDS_AlphaBlend turns up.
        DrawStyle::AlphaBlend => Blend::AlphaAdditive,
        DrawStyle::AlphaModulate => Blend::Alpha,
        DrawStyle::Translucent => Blend::Translucent,
        DrawStyle::Modulated => Blend::Modulate,
        DrawStyle::Darken => Blend::Darken,
        DrawStyle::Brighten => Blend::Brighten,
    }
}

/// Right and up axes of the plane perpendicular to `normal`, or `None` for a zero normal. As Unreal's
/// sprite emitter builds them (recovered by Fermata): up is `normal × normal.GetNonParallel()`, which
/// keeps its length and so shortens sprites on tilted planes, and right is `normal × up`, normalized.
/// Fermata writes this in its Y-up viewer space, where Unreal's Y and Z swap, so the math runs there.
fn plane_axes(normal: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
    let length = normal.iter().map(|axis| axis * axis).sum::<f32>().sqrt();
    if length < 1e-4 {
        return None;
    }
    let swap = |[x, y, z]: [f32; 3]| [x, z, y];
    let n = swap(normal.map(|axis| axis / length));
    let non_parallel = if n[0].abs() > 0.57 {
        [0.0, 0.0, 1.0]
    } else if n[2].abs() > 0.57 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let up = cross(n, non_parallel);
    let right = normalized(cross(n, up));
    Some((swap(right), swap(up)))
}

fn cross([ax, ay, az]: [f32; 3], [bx, by, bz]: [f32; 3]) -> [f32; 3] {
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

fn normalized(vector: [f32; 3]) -> [f32; 3] {
    let length = vector.iter().map(|axis| axis * axis).sum::<f32>().sqrt().max(1e-6);
    vector.map(|axis| axis / length)
}

/// `first * a + second * b`.
fn mix(first: [f32; 3], second: [f32; 3], a: f32, b: f32) -> [f32; 3] {
    let mut out = first;
    for (out, second) in out.iter_mut().zip(second) {
        *out = *out * a + second * b;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_planes_follow_their_normal() {
        // A normal along Y: GetNonParallel gives Z, so up runs along -X and right along -Z.
        let (right, up) = plane_axes([0.0, 2.0, 0.0]).unwrap();
        assert_eq!((right, up), ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0]));
        assert!(plane_axes([0.0; 3]).is_none());
    }

    #[test]
    fn revolution_turns_about_each_axis() {
        // A quarter turn about Y carries a point above the emitter onto X.
        let [x, y, z] = revolve([0.0, 0.0, 128.0], [0.0, 0.25, 0.0], 1.0);
        assert!((x - 128.0).abs() < 1e-3 && y.abs() < 1e-3 && z.abs() < 1e-3);
    }
}

//! Sprite particles simulated on the CPU the way Unreal Engine 2's sprite emitters behave: spawned in a
//! box around the emitter, moving with velocity and acceleration, sized and colored by curves over
//! their life, fading in and out, spinning, and respawning when they die.

use ue2_level::{DrawStyle, Emitter, SpriteEmitter};

use super::camera;
use super::curves::{color_at, fade, size_at};
use super::load::Vertex;
use super::random::Random;

pub(crate) struct System {
    pub(crate) sprite: SpriteEmitter,
    /// The owning emitter's location relative to the camera.
    origin: [f32; 3],
    particles: Vec<Particle>,
    random: Random,
}

struct Particle {
    /// Scene time of birth; negative for particles alive when the scene starts.
    born: f32,
    lifetime: f32,
    start: [f32; 3],
    velocity: [f32; 3],
    size: f32,
    /// Starting turn and turns per second.
    spin: [f32; 2],
    color: [f32; 3],
    /// Texture cell, row by row.
    cell: u32,
}

/// One system per sprite emitter, started as if it had been running a whole lifetime, as Unreal's
/// warmup does.
pub(crate) fn start(emitters: &[Emitter], camera: [f32; 3]) -> Vec<System> {
    let mut seed = 0x9E37_79B9_7F4A_7C15;
    emitters
        .iter()
        .flat_map(|emitter| emitter.sprites.iter().map(move |sprite| (emitter, sprite)))
        .filter(|(_, sprite)| sprite.texture.is_some())
        .map(|(emitter, sprite)| {
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
            System { sprite: sprite.clone(), origin, particles, random }
        })
        .collect()
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

    /// Four corners per particle at `time`, facing the camera given by its `rotation` unless the
    /// emitter lays sprites in a plane.
    pub(crate) fn quads(&self, time: f32, rotation: [i32; 3], vertices: &mut Vec<Vertex>) {
        let [_, camera_right, camera_up] = camera::axes(rotation);
        let (right, up) = self.sprite.projection_normal.and_then(plane_axes).unwrap_or((camera_right, camera_up));
        let [columns, rows] = self.sprite.subdivisions;
        for particle in &self.particles {
            let age = (time - particle.born).max(0.0);
            let life = (age / particle.lifetime).clamp(0.0, 1.0);
            let center = self.origin_plus(particle, age);
            let size = particle.size * size_at(&self.sprite, life);
            let mut color = color_at(&self.sprite, life);
            for (channel, multiplier) in color.iter_mut().zip(particle.color) {
                *channel *= multiplier;
            }
            color[3] *= self.sprite.opacity * fade(&self.sprite, age, particle.lifetime);
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
        let mut at = self.origin;
        for (((at, start), velocity), acceleration) in
            at.iter_mut().zip(particle.start).zip(particle.velocity).zip(self.sprite.acceleration)
        {
            *at += start + velocity * age + 0.5 * acceleration * age * age;
        }
        at
    }

    /// Particles this system draws, which is also how many quads `quads` writes.
    pub(crate) fn len(&self) -> usize {
        self.particles.len()
    }
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
        spin: sprite.spin.map_or([0.0; 2], |(start, rate)| [random.range(start), random.range(rate)]),
        color: sprite.color_multiplier.map(|range| random.range(range)),
        cell: first + (random.unit() * span as f32) as u32 % span,
    }
}

/// How the emitter's draw style blends.
pub(crate) fn blend(style: DrawStyle) -> l2_catalog::Blend {
    use l2_catalog::Blend;
    match style {
        DrawStyle::Regular => Blend::Opaque,
        DrawStyle::AlphaBlend | DrawStyle::AlphaModulate => Blend::Alpha,
        DrawStyle::Translucent => Blend::Additive,
        DrawStyle::Modulated => Blend::Modulate,
        DrawStyle::Darken => Blend::Darken,
        DrawStyle::Brighten => Blend::Brighten,
    }
}

/// Right and up axes of the plane perpendicular to `normal`, or `None` for a zero normal.
fn plane_axes(normal: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
    let length = normal.iter().map(|axis| axis * axis).sum::<f32>().sqrt();
    if length < 1e-4 {
        return None;
    }
    let [x, y, z] = normal.map(|axis| axis / length);
    // Right lies in the ground plane unless the normal points straight up.
    let right = if x.abs() + y.abs() < 1e-4 { [1.0, 0.0, 0.0] } else { normalized([-y, x, 0.0]) };
    let up = normalized([y * right[2] - z * right[1], z * right[0] - x * right[2], x * right[1] - y * right[0]]);
    Some((right, up))
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
        let (right, up) = plane_axes([0.0, 2.0, 0.0]).unwrap();
        assert_eq!((right, up), ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
        assert!(plane_axes([0.0; 3]).is_none());
    }
}

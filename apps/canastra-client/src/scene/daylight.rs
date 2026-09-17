//! Time-of-day lighting for world zones, whose meshes carry no precomputed colors: a hemisphere ambient and a
//! wrapped sun diffuse, fed by the client's `TimeEnv` ramps. Vertices carry their normal and the ramp that lights
//! them, and the scene shader applies the hour's light, so the hour can change without rebuilding geometry.
// ponytail: no sun shadows on static meshes (each instance's visibility bits), rim or specular yet; add them as the
// comparison with H5 shows they matter.

use std::sync::OnceLock;
use std::time::Instant;

use l2_env::Environment;
use ue2_level::Actor;

use super::camera;
use super::load::Vertex;

/// How much of the sky light the ground gives back to a surface facing it.
const BOUNCE: f32 = 0.54;

/// The kinds of surface the client lights with ramps of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ramp {
    StaticMesh = 1,
    Terrain = 2,
    Bsp = 3,
    Actor = 4,
    /// The sky dome: haze at the horizon to the sky color above, by how far up the vertex stands.
    Sky = 5,
    Clouds = 6,
}

/// Each ramp's ambient color and HSV light sections in `TimeEnv`, in `Ramp` order.
const SECTIONS: [[&str; 2]; 4] = [
    ["StaticMeshAmbient", "HSVStaticMeshLight"],
    ["TerrainAmbient", "HSVTerrainLight"],
    ["BSPAmbient", "HSVBSPLight"],
    ["ActorAmbient", "HSVActorLight"],
];

/// The sky's color sections: the haze at the horizon, the sky above, and the wispy cloud layer the H5 creation
/// screenshot shows, which `Env.int` lists fourth.
const SKY: [&str; 3] = ["HazeringColor", "SkyBoxColor", "CloudColor4"];

/// When the lobby clock started: the first time any scene asked for the hour, as H5's starts with the client.
static CLOCK: OnceLock<Instant> = OnceLock::new();

/// Real seconds since the lobby clock started.
pub(super) fn clock_seconds() -> f32 {
    CLOCK.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Sky light at one hour and where the sun shines from.
pub(super) struct Daylight {
    hour: f32,
    /// Unit vector pointing toward the sun.
    toward_sun: [f32; 3],
    /// For each ramp, the ambient color, the sun's color, and what a surface facing the ground gets back from it:
    /// a share of the sky light, in the ground's own colour.
    ramps: [[[f32; 3]; 3]; 4],
    /// The haze, sky and cloud colors.
    sky: [[f32; 3]; 3],
}

impl Daylight {
    /// Which way the level's `NMovableSunLight` shines from, when it has one.
    // ponytail: the sun keeps the direction the level saved; H5 moves it with the hour in native code.
    pub(super) fn toward_sun(actors: &[Actor]) -> Option<[f32; 3]> {
        let sun = actors.iter().find(|actor| actor.class.eq_ignore_ascii_case("NMovableSunLight"))?;
        let [forward, _, _] = camera::axes(sun.placement.rotation);
        Some(forward.map(|axis| -axis))
    }

    /// The light of every kind of surface and the sky's colors at `hour`, from the client's ramps, with the sun
    /// shining from `toward_sun`.
    pub(super) fn new(environment: &Environment, hour: f32, toward_sun: [f32; 3]) -> Option<Self> {
        let unit = |color: [u8; 3]| color.map(|channel| f32::from(channel) / 255.0);
        // The ground gives back about half the sky, in the colour the client paints the ground with at this hour.
        let ground = unit(environment.color("TerrainAmbient", hour).unwrap_or([255; 3]));
        let brightest = ground.iter().copied().fold(f32::EPSILON, f32::max);
        let bounce = ground.map(|channel| channel / brightest * BOUNCE);
        let mut ramps = [[[0.0; 3]; 3]; 4];
        for (ramp, [ambient, light]) in ramps.iter_mut().zip(SECTIONS) {
            *ramp = [unit(environment.color(ambient, hour)?), unit(environment.light(light, hour)?), bounce];
        }
        let mut sky = [[0.0; 3]; 3];
        for (color, section) in sky.iter_mut().zip(SKY) {
            *color = unit(environment.color(section, hour)?);
        }
        Some(Self { hour, toward_sun, ramps, sky })
    }

    pub(super) fn hour(&self) -> f32 {
        self.hour
    }

    /// Which of the eight time-of-day states a level stores, three hours each, holds this hour.
    // ponytail: states assumed to start at midnight, in order; check them against more H5 screenshots.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0..8")]
    pub(super) fn time_slot(&self) -> u8 {
        (self.hour / 3.0).clamp(0.0, 7.0) as u8
    }

    /// The shader's light, as `vec4`s: toward the sun, each ramp's ambient, sun and bounce, then the haze, sky and
    /// cloud colors; see `Globals` in `scene.wgsl`.
    pub(super) fn uniform(&self) -> [[f32; 4]; 16] {
        let mut rows = [[0.0; 4]; 16];
        let colors = self.ramps.iter().flatten().chain(&self.sky);
        for (row, [x, y, z]) in rows.iter_mut().zip(std::iter::once(&self.toward_sun).chain(colors)) {
            *row = [*x, *y, *z, 0.0];
        }
        rows
    }
}

/// A vertex the hour lights through `ramp`, on top of the `base` light the level stored for it, facing `normal` in
/// world space and receiving `sunlit` of the sun and `skylit` of the sky, each from 0 to 1.
#[expect(clippy::too_many_arguments, reason = "one vertex's fields, named where they are built")]
pub(super) fn lit(
    at: [f32; 3],
    uv: [f32; 2],
    base: [f32; 3],
    alpha: f32,
    normal: [f32; 3],
    ramp: Ramp,
    sunlit: f32,
    skylit: f32,
) -> Vertex {
    let [x, y, z] = at;
    let [red, green, blue] = base;
    let [nx, ny, nz] = normal;
    [x, y, z, uv[0], uv[1], red, green, blue, alpha, nx, ny, nz, f32::from(ramp as u8), sunlit, skylit]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uniform_leads_with_the_sun_then_each_ramp_in_order() {
        let mut ramps = [[[0.0; 3]; 3]; 4];
        ramps[3][1] = [0.6, 0.4, 0.3];
        let daylight = Daylight { hour: 21.0, toward_sun: [0.0, 0.0, 1.0], ramps, sky: [[0.5; 3]; 3] };
        let rows = daylight.uniform();
        let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-6);
        assert!(close(rows[0], [0.0, 0.0, 1.0, 0.0]));
        assert!(close(rows[1 + (Ramp::Actor as usize - 1) * 3 + 1], [0.6, 0.4, 0.3, 0.0]), "the actor ramp's sun");
        assert!(close(rows[15], [0.5, 0.5, 0.5, 0.0]), "the cloud color comes last");
        assert_eq!(daylight.time_slot(), 7);
    }
}

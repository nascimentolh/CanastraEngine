//! The sky of world zones, which levels leave to the engine: a dome shading from the haze color at the horizon
//! to the sky color above, under a layer of drifting clouds. `system/Env.int` names the materials and
//! `TimeEnv` colors them by the hour.

use l2_catalog::{Catalog, Material};
use l2_env::Environment;

use super::daylight::{self, Ramp};
use super::load::Group;

/// Far enough to stand behind everything a lobby scene draws.
const DOME_RADIUS: f32 = 200_000.0;
const CLOUD_RADIUS: f32 = 190_000.0;
const SEGMENTS: usize = 32;
const RINGS: usize = 12;
/// Elevation, in radians, the dome reaches below the horizon so no gap shows under hills.
const BELOW_HORIZON: f32 = 0.15;
const WHITE: &str = "L2_Skies.Textures.WhiteChip";
/// The wispy layer `Env.int` lists fourth, colored by `CloudColor4`; the H5 creation screenshot shows it.
const CLOUDS: &str = "L2_Skies.Shaders.Cloud_Final01_sh";
/// Cloud texture repeats across the dome.
// ponytail: one cloud layer, its scale and horizon fade set by eye against the H5 creation screenshot.
const CLOUD_REPEATS: f32 = 1.0;

/// The dome and the cloud layer, back to front, with their materials, colored by the hour in the shader; empty
/// when the client lacks them.
pub(super) fn groups(catalog: &mut Catalog, environment: &Environment) -> Vec<(Material, Group)> {
    if ["SkyBoxColor", "HazeringColor"].iter().any(|section| environment.color(section, 0.0).is_none()) {
        return Vec::new();
    }
    // The dome shades from the haze to the sky over its lowest part.
    let gradient = dome(DOME_RADIUS, Ramp::Sky, |up, _| ((up / 0.35).clamp(0.0, 1.0), 1.0, [0.0; 2]));
    let clouds = dome(CLOUD_RADIUS, Ramp::Clouds, |up, [x, y]| {
        // Clouds lie on a flattened dome: their texture spreads toward the horizon and fades there.
        let spread = CLOUD_REPEATS / (up.max(0.0) + 0.25);
        (0.0, (up / 0.2).clamp(0.0, 1.0), [x * spread, y * spread])
    });
    [(WHITE, gradient), (CLOUDS, clouds)]
        .into_iter()
        .filter_map(|(path, group)| {
            let mut material = catalog.material(path)?;
            // The hour's color replaces the one the material was saved with.
            material.color = [255; 4];
            Some((material, group))
        })
        .collect()
}

/// A dome around the camera colored by `ramp`. `shade` gives each vertex how far from the haze to the sky it
/// stands, its alpha and its UV, from how far up it points, from -1 to 1, and its horizontal direction.
fn dome(radius: f32, ramp: Ramp, shade: impl Fn(f32, [f32; 2]) -> (f32, f32, [f32; 2])) -> Group {
    let mut group = Group::default();
    for ring in 0..=RINGS {
        let elevation = -BELOW_HORIZON + (std::f32::consts::FRAC_PI_2 + BELOW_HORIZON) * ring as f32 / RINGS as f32;
        for segment in 0..=SEGMENTS {
            let azimuth = std::f32::consts::TAU * segment as f32 / SEGMENTS as f32;
            let across = [elevation.cos() * azimuth.cos(), elevation.cos() * azimuth.sin()];
            let up = elevation.sin();
            let (height, alpha, uv) = shade(up, across);
            let at = [across[0] * radius, across[1] * radius, up * radius];
            group.vertices.push(daylight::lit(at, uv, [0.0; 3], alpha, [0.0; 3], ramp, height, 0.0));
        }
    }
    let columns = SEGMENTS + 1;
    for ring in 0..RINGS {
        for segment in 0..SEGMENTS {
            let corner = |dr: usize, ds: usize| u32::try_from((ring + dr) * columns + segment + ds).unwrap_or(0);
            group.indices.extend([corner(0, 0), corner(1, 0), corner(0, 1), corner(0, 1), corner(1, 0), corner(1, 1)]);
        }
    }
    group
}

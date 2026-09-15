//! The sky of world zones, which levels leave to the engine: a dome shading from the haze color at the horizon
//! to the sky color above, under a layer of drifting clouds. `system/Env.int` names the materials and
//! `TimeEnv` colors them by the hour.

use l2_catalog::{Catalog, Material};
use l2_env::Environment;

use super::load::{Group, Vertex};

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
const CLOUD_COLOR: &str = "CloudColor4";
/// Cloud texture repeats across the dome.
// ponytail: one cloud layer, its scale and horizon fade set by eye against the H5 creation screenshot.
const CLOUD_REPEATS: f32 = 1.0;

/// The dome and the cloud layer at `hour`, back to front, with their materials; empty when the client lacks them.
pub(super) fn groups(catalog: &mut Catalog, environment: &Environment, hour: f32) -> Vec<(Material, Group)> {
    let unit = |color: [u8; 3]| color.map(|channel| f32::from(channel) / 255.0);
    let (Some(sky), Some(haze), Some(cloud)) = (
        environment.color("SkyBoxColor", hour).map(unit),
        environment.color("HazeringColor", hour).map(unit),
        environment.color(CLOUD_COLOR, hour).map(unit),
    ) else {
        return Vec::new();
    };
    let gradient = dome(DOME_RADIUS, |up, _| {
        let height = (up / 0.35).clamp(0.0, 1.0);
        let mut color = [0.0; 4];
        for ((out, haze), sky) in color.iter_mut().zip(haze).zip(sky) {
            *out = haze + (sky - haze) * height;
        }
        color[3] = 1.0;
        (color, [0.0; 2])
    });
    let clouds = dome(CLOUD_RADIUS, |up, [x, y]| {
        // Clouds lie on a flattened dome: their texture spreads toward the horizon and fades there.
        let spread = CLOUD_REPEATS / (up.max(0.0) + 0.25);
        let fade = (up / 0.2).clamp(0.0, 1.0);
        ([cloud[0], cloud[1], cloud[2], fade], [x * spread, y * spread])
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

/// A dome around the camera. `shade` gives each vertex its RGBA and UV from how far up it points, from -1 to
/// 1, and its horizontal direction.
fn dome(radius: f32, shade: impl Fn(f32, [f32; 2]) -> ([f32; 4], [f32; 2])) -> Group {
    let mut group = Group::default();
    for ring in 0..=RINGS {
        let elevation = -BELOW_HORIZON + (std::f32::consts::FRAC_PI_2 + BELOW_HORIZON) * ring as f32 / RINGS as f32;
        for segment in 0..=SEGMENTS {
            let azimuth = std::f32::consts::TAU * segment as f32 / SEGMENTS as f32;
            let across = [elevation.cos() * azimuth.cos(), elevation.cos() * azimuth.sin()];
            let up = elevation.sin();
            let ([red, green, blue, alpha], [u, v]) = shade(up, across);
            let vertex: Vertex = [across[0] * radius, across[1] * radius, up * radius, u, v, red, green, blue, alpha];
            group.vertices.push(vertex);
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

//! Terrain decoration layers: a static mesh, such as grass, scattered over the quads a density map
//! paints, lit by the terrain under it.

use l2_catalog::Catalog;
use ue2_level::{Actor, DecoLayer, Placement, Terrain};

use super::random::Random;
use super::terrain::{ZERO_HEIGHT, intensities};

/// Every decoration within fade-out range of `camera` as a placed static mesh actor with its opacity,
/// which falls from 1 to 0 across the layer's fade-out radii as Fermata fades them.
#[expect(clippy::cast_sign_loss, reason = "terrain intensities lie between 0 and 1")]
pub(super) fn actors(
    terrains: &[Terrain],
    catalog: &mut Catalog,
    camera: [f32; 3],
    zone_state: Option<u8>,
) -> Vec<(Actor, f32)> {
    let mut actors = Vec::new();
    for terrain in terrains {
        let Some(map) = catalog.heightmap(&terrain.heightmap) else { continue };
        let light = intensities(terrain, map.width, map.height, zone_state);
        for layer in &terrain.deco_layers {
            let (Some(density), Some(mesh)) =
                (catalog.texture(&layer.density_map), catalog.static_mesh(&layer.static_mesh))
            else {
                continue;
            };
            let vertices = mesh.mesh.positions.len();
            let mut random = Random(u64::from(layer.seed.cast_unsigned()) + 0x9E37_79B9);
            for (x, y, fraction) in placements(layer, &density, map.width, map.height, &mut random) {
                let quad = y * map.width + x;
                if !layer.on_invisible_terrain
                    && terrain.visible_quads.get(quad / 32).is_some_and(|word| word >> (quad % 32) & 1 == 0)
                {
                    continue;
                }
                let sample = |dx: usize, dy: usize| {
                    map.samples.get((y + dy) * map.width + x + dx).map_or(ZERO_HEIGHT, |&sample| f32::from(sample))
                };
                let [fx, fy] = fraction;
                let height = (sample(0, 0) * (1.0 - fx) + sample(1, 0) * fx) * (1.0 - fy)
                    + (sample(0, 1) * (1.0 - fx) + sample(1, 1) * fx) * fy;
                let location = [
                    terrain.location[0] + (x as f32 + fx - map.width as f32 / 2.0) * terrain.scale[0],
                    terrain.location[1] + (y as f32 + fy - map.height as f32 / 2.0) * terrain.scale[1],
                    terrain.location[2] + (height - ZERO_HEIGHT) / 256.0 * terrain.scale[2],
                ];
                let scale = layer.scale.map(|range| random.range(range));
                let yaw = if layer.random_yaw { (random.unit() * 65536.0) as i32 } else { 0 };
                let distance = location.iter().zip(camera).map(|(at, eye)| (at - eye) * (at - eye)).sum::<f32>().sqrt();
                let [near, far] = layer.fadeout_radius;
                if distance >= far {
                    continue;
                }
                let opacity = 1.0 - ((distance - near) / (far - near).max(1.0)).clamp(0.0, 1.0);
                let bright = (light.get(quad).copied().unwrap_or(1.0) * 255.0) as u8;
                let actor = Actor {
                    class: "TerrainDecoration".to_owned(),
                    name: String::new(),
                    tag: None,
                    groups: Vec::new(),
                    placement: Placement { location, rotation: [0, yaw, 0] },
                    scale,
                    static_mesh: Some(layer.static_mesh.clone()),
                    skins: Vec::new(),
                    unlit: false,
                    lighting: vec![[bright, bright, bright, 255]; vertices],
                };
                actors.push((actor, opacity));
            }
        }
    }
    actors
}

/// The quad and the point inside it of each decoration `layer` places: every quad offers
/// `max_per_quad` chances, each taken with the density map's weight times the layer's multiplier, a
/// percentage.
// ponytail: Unreal's placement stream is unknown; the percentage reading matches the H5 login's sparse tufts, not its exact spots.
fn placements(
    layer: &DecoLayer,
    density: &ue2_assets::Image,
    width: usize,
    height: usize,
    random: &mut Random,
) -> Vec<(usize, usize, [f32; 2])> {
    let (map_width, map_height) = (density.width as usize, density.height as usize);
    let mut placements = Vec::new();
    for y in 0..height.saturating_sub(1) {
        for x in 0..width.saturating_sub(1) {
            let texel = (y * map_height / height) * map_width + x * map_width / width;
            let weight = f32::from(density.rgba.get(texel * 4).copied().unwrap_or(0)) / 255.0;
            if weight == 0.0 {
                continue;
            }
            for _ in 0..layer.max_per_quad {
                if random.unit() < weight * random.range(layer.density_multiplier) / 100.0 {
                    placements.push((x, y, [random.unit(), random.unit()]));
                }
            }
        }
    }
    placements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_painted_quads_get_decorations() {
        let layer = DecoLayer {
            static_mesh: String::new(),
            density_map: String::new(),
            density_multiplier: [100.0; 2],
            max_per_quad: 2,
            scale: [[1.0; 2]; 3],
            fadeout_radius: [0.0; 2],
            seed: 0,
            random_yaw: false,
            on_invisible_terrain: false,
        };
        // A density map as large as the 3×3 heightmap: only the second quad of the first row is painted.
        let mut rgba = vec![0; 36];
        rgba[4] = 255;
        let density = ue2_assets::Image { width: 3, height: 3, rgba };
        let placed = placements(&layer, &density, 3, 3, &mut Random(1));
        assert_eq!(placed.len(), 2);
        assert!(
            placed
                .iter()
                .all(|&(x, y, [fx, fy])| (x, y) == (1, 0) && (0.0..1.0).contains(&fx) && (0.0..1.0).contains(&fy))
        );
    }
}

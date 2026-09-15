//! Terrain geometry: one vertex per heightmap sample, holes left out, and one batch per painted layer.

use l2_catalog::{Blend, Catalog, Combine, Heightmap, Material, Stage, UvModifier};
use ue2_level::Terrain;

use super::load::{Group, Vertex};

/// Heights are stored around this middle value.
pub(super) const ZERO_HEIGHT: f32 = 32768.0;

/// The terrains' layers, bottom first, each with the geometry it paints.
pub(super) fn groups(
    terrains: &[Terrain],
    catalog: &mut Catalog,
    camera: [f32; 3],
    zone_state: Option<u8>,
) -> Vec<(Material, Group)> {
    let mut groups = Vec::new();
    for terrain in terrains {
        let Some(map) = catalog.heightmap(&terrain.heightmap) else { continue };
        let (width, height) = (map.width, map.height);
        let Group { vertices, indices } = geometry(terrain, &map, camera, zone_state);
        for (index, layer) in terrain.layers.iter().enumerate() {
            let Some(mut material) = catalog.material(&layer.material) else { continue };
            material.base.uv.insert(0, UvModifier::Scale { scale: layer.scale, offset: [0.0; 2] });
            // The bottom layer covers the ground; the ones above blend in where their alpha map is bright.
            if let (true, Some(alpha_map)) = (index > 0, &layer.alpha_map) {
                let whole = UvModifier::Scale { scale: [width as f32, height as f32], offset: [0.0; 2] };
                let stage = Stage { texture: alpha_map.clone(), uv: vec![whole] };
                material.layer = Some((stage, Combine::Mask, 1.0));
                material.blend = Blend::Alpha;
            }
            groups.push((material, Group { vertices: vertices.clone(), indices: indices.clone() }));
        }
    }
    groups
}

/// One vertex per heightmap sample relative to `camera`, with texture coordinates in samples, and two
/// triangles per visible quad.
fn geometry(terrain: &Terrain, map: &Heightmap, camera: [f32; 3], zone_state: Option<u8>) -> Group {
    let (width, height) = (map.width, map.height);
    let light = intensities(terrain, width, height, zone_state);
    let [scale_x, scale_y, scale_z] = terrain.scale;
    let mut vertices: Vec<Vertex> = Vec::with_capacity(width * height);
    for (index, &sample) in map.samples.iter().take(width * height).enumerate() {
        let (x, y) = ((index % width) as f32, (index / width) as f32);
        let world = [
            terrain.location[0] + (x - width as f32 / 2.0) * scale_x,
            terrain.location[1] + (y - height as f32 / 2.0) * scale_y,
            terrain.location[2] + (f32::from(sample) - ZERO_HEIGHT) / 256.0 * scale_z,
        ];
        let bright = light.get(index).copied().unwrap_or(1.0);
        vertices.push([
            world[0] - camera[0],
            world[1] - camera[1],
            world[2] - camera[2],
            x,
            y,
            bright,
            bright,
            bright,
            1.0,
        ]);
    }
    // ponytail: every quad splits along the same diagonal; read EdgeTurnBitmap if seams show.
    let mut indices = Vec::new();
    for quad in 0..width * height {
        let (x, y) = (quad % width, quad / width);
        let visible = terrain.visible_quads.get(quad / 32).is_none_or(|word| word >> (quad % 32) & 1 == 1);
        if x + 1 == width || y + 1 == height || !visible {
            continue;
        }
        let corner = |dx, dy| u32::try_from((y + dy) * width + x + dx).unwrap_or(u32::MAX);
        indices.extend([corner(0, 0), corner(0, 1), corner(1, 0), corner(1, 0), corner(0, 1), corner(1, 1)]);
    }
    Group { vertices, indices }
}

/// Each vertex's precomputed intensity for the zone's state, from the sector covering it; vertices no
/// sector covers stay fully lit.
pub(super) fn intensities(terrain: &Terrain, width: usize, height: usize, zone_state: Option<u8>) -> Vec<f32> {
    let mut light = vec![1.0; width * height];
    let state = usize::from(zone_state.unwrap_or(0));
    for sector in &terrain.sectors {
        let Some(map) = sector.intensities.get(state).or(sector.intensities.last()) else { continue };
        let [offset_x, offset_y] = sector.offset.map(|offset| offset as usize);
        let columns = sector.quads[0] as usize + 1;
        for (index, &value) in map.iter().enumerate() {
            let (x, y) = (offset_x + index % columns, offset_y + index / columns);
            if x < width
                && let Some(vertex) = light.get_mut(y * width + x)
            {
                *vertex = f32::from(value) / 255.0;
            }
        }
    }
    light
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(clippy::float_cmp, reason = "every value is an exact binary fraction")]
    fn places_samples_and_leaves_holes_out() {
        let terrain = Terrain {
            location: [100.0, 0.0, 10.0],
            scale: [64.0, 64.0, 256.0],
            heightmap: String::new(),
            layers: Vec::new(),
            // Quads are numbered row by row over the full width: 0 and 1 in the first row, 3 and 4 in
            // the second. Quad 4 is a hole.
            visible_quads: vec![!(1 << 4)],
            sectors: Vec::new(),
            deco_layers: Vec::new(),
        };
        let map = Heightmap {
            width: 3,
            height: 3,
            samples: vec![32768, 32769, 32768, 32768, 32768, 32768, 32768, 32768, 32768],
        };
        let Group { vertices, indices } = geometry(&terrain, &map, [100.0, 0.0, 0.0], None);

        // The heightmap's center (1.5 samples in) sits on the terrain's location; one height step is
        // scale_z / 256.
        assert_eq!(vertices[4], [-32.0, -32.0, 10.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        assert_eq!(vertices[1], [-32.0, -96.0, 11.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
        assert_eq!(indices.len(), 3 * 6);
        assert_eq!(indices[12..18], [3, 6, 4, 4, 6, 7]);
    }

    #[test]
    #[expect(clippy::float_cmp, reason = "every value is an exact binary fraction")]
    fn sectors_light_the_vertices_they_cover_for_the_zone_state() {
        let sector =
            ue2_level::TerrainSector { offset: [1, 0], quads: [1, 1], intensities: vec![vec![0; 4], vec![51; 4]] };
        let terrain = Terrain {
            location: [0.0; 3],
            scale: [1.0; 3],
            heightmap: String::new(),
            layers: Vec::new(),
            visible_quads: Vec::new(),
            sectors: vec![sector],
            deco_layers: Vec::new(),
        };
        // A 3×2 grid: the sector covers columns 1 and 2 of both rows; column 0 stays lit.
        assert_eq!(intensities(&terrain, 3, 2, Some(1)), [1.0, 0.2, 0.2, 1.0, 0.2, 0.2]);
        assert_eq!(intensities(&terrain, 3, 2, Some(9))[1], 0.2);
    }
}

//! The level's BSP polygons as scene geometry: fanned into triangles, textured along their surface's texture
//! axes, and grouped by material.

use std::collections::HashMap;

use l2_catalog::{Catalog, Material};
use ue2_level::BspPolygon;

use super::daylight::Daylight;
use super::load::Group;

/// The polygons' groups by material, relative to `camera`. World zones take `daylight`; elsewhere polygons
/// draw at full brightness.
// ponytail: BSP light maps are not read yet; add them where zones rely on precomputed light.
pub(super) fn groups(
    polygons: &[BspPolygon],
    catalog: &mut Catalog,
    camera: [f32; 3],
    daylight: Option<&Daylight>,
) -> Vec<(Material, Group)> {
    let mut groups: HashMap<String, (Material, [f32; 2], Group)> = HashMap::new();
    for polygon in polygons {
        let Some(path) = &polygon.material else { continue };
        if !groups.contains_key(path) {
            let Some(material) = catalog.material(path) else { continue };
            let size = catalog.texture_size(&material.base.texture).unwrap_or([512.0; 2]);
            groups.insert(path.clone(), (material, size, Group::default()));
        }
        let Some((_, size, group)) = groups.get_mut(path) else { continue };
        let normal = normal(&polygon.corners);
        let light = daylight.map_or([1.0; 3], |daylight| daylight.on_shaded(normal, 1.0, 1.0));
        let first = u32::try_from(group.vertices.len()).unwrap_or(u32::MAX);
        for &corner in &polygon.corners {
            let offset: Vec<f32> =
                corner.iter().zip(polygon.texture_origin).map(|(corner, origin)| corner - origin).collect();
            let along = |axis: [f32; 3]| offset.iter().zip(axis).map(|(offset, axis)| offset * axis).sum::<f32>();
            let uv = [along(polygon.texture_axes[0]) / size[0], along(polygon.texture_axes[1]) / size[1]];
            let at = [corner[0] - camera[0], corner[1] - camera[1], corner[2] - camera[2]];
            group.vertices.push([at[0], at[1], at[2], uv[0], uv[1], light[0], light[1], light[2], 1.0]);
        }
        let count = u32::try_from(polygon.corners.len()).unwrap_or(0);
        for corner in 1..count.saturating_sub(1) {
            group.indices.extend([first, first + corner + 1, first + corner]);
        }
    }
    groups.into_values().map(|(material, _, group)| (material, group)).collect()
}

/// The facing of a convex polygon, from its first three corners.
fn normal(corners: &[[f32; 3]]) -> [f32; 3] {
    let [Some(origin), Some(second), Some(third)] = [corners.first(), corners.get(1), corners.get(2)] else {
        return [0.0, 0.0, 1.0];
    };
    let edge = |corner: &[f32; 3]| [corner[0] - origin[0], corner[1] - origin[1], corner[2] - origin[2]];
    let (along, across) = (edge(second), edge(third));
    [
        along[1] * across[2] - along[2] * across[1],
        along[2] * across[0] - along[0] * across[2],
        along[0] * across[1] - along[1] * across[0],
    ]
}

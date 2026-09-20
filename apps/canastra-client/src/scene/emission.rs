//! How particle systems draw: one batch for a system's sprites or one for each section of its mesh, over
//! indices laid out once for all of its particles, and the vertices every system writes each frame.

use std::ops::Range;

use l2_catalog::{Material, Stage};

use super::Batch;
use super::particles::{self, System};
use super::pipeline::Draw;

/// What laying out the particle systems gives: the batches they draw with, where each system's vertices
/// begin, how many vertices they write in all, and the indices over them.
pub(super) struct Laid {
    pub(super) batches: Vec<Batch>,
    pub(super) starts: Vec<usize>,
    pub(super) vertices: usize,
    pub(super) indices: Vec<u32>,
}

/// The batches `systems` draw with, made by `batch`.
pub(super) fn layout(
    systems: &[System],
    batch: &mut dyn FnMut(Material, Draw, bool, f32, Range<u32>) -> Option<Batch>,
) -> Result<Laid, String> {
    // Every particle system's vertices follow the last one's; each draws its sprites, or its mesh's sections,
    // over indices laid out for all of its particles.
    let (mut vertex_count, mut indices_all) = (0_usize, Vec::<u32>::new());
    let (mut batches, mut starts) = (Vec::new(), Vec::with_capacity(systems.len()));
    for (index, system) in systems.iter().enumerate() {
        starts.push(vertex_count);
        let sprite = &system.sprite;
        let base = u32::try_from(vertex_count).map_err(|_| "too many particles")?;
        let each = u32::try_from(system.vertices_each()).map_err(|_| "too many particles")?;
        let particles = u32::try_from(system.len()).map_err(|_| "too many particles")?;
        let lay = |pattern: &[u32]| -> Vec<u32> {
            (0..particles)
                .flat_map(|particle| pattern.iter().map(move |index| base + particle * each + index))
                .collect()
        };
        let own_blend = sprite.mesh.as_ref().is_some_and(|shape| shape.own_blend);
        let sections: Vec<(Material, Vec<u32>)> = if let Some(mesh) = &system.mesh {
            mesh.sections
                .iter()
                .map(|(material, indices)| {
                    let blend = if own_blend { material.blend } else { particles::blend(sprite.draw_style) };
                    (Material { blend, ..material.clone() }, lay(indices))
                })
                .collect()
        } else {
            let material = Material {
                base: Stage { texture: sprite.texture.clone().unwrap_or_default(), uv: Vec::new() },
                frames: Vec::new(),
                fps: 0.0,
                layer: None,
                blend: particles::blend(sprite.draw_style),
                color: [255; 4],
                fade: None,
                glow: None,
                alpha_ref: None,
            };
            vec![(material, lay(&[0, 1, 2, 0, 2, 3]))]
        };
        vertex_count += system.len() * system.vertices_each();
        // ponytail: soft sprites fade over half their mean size; tune against the H5 login if edges show.
        let soft = if sprite.soft && system.mesh.is_none() {
            (sprite.start_size[0] + sprite.start_size[1]) / 4.0
        } else {
            0.0
        };
        for (material, indices) in sections {
            let draw = Draw { blend: material.blend, depth_test: sprite.z_test, depth_write: false };
            let start = u32::try_from(indices_all.len()).map_err(|_| "too many particles")?;
            indices_all.extend(indices);
            let end = u32::try_from(indices_all.len()).map_err(|_| "too many particles")?;
            let zone = system.zone.clone();
            let made = batch(material, draw, sprite.fogged, soft, start..end);
            batches.extend(made.map(|b| Batch { zone, system: Some(index), ..b }));
        }
    }
    Ok(Laid { batches, starts, vertices: vertex_count, indices: indices_all })
}

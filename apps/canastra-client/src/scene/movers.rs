//! Static meshes that sway, such as the Kamael hall's floating stones: drawn from the particle buffers, since
//! their vertices are placed again every frame.

use std::ops::Range;

use l2_catalog::{Catalog, Material};
use ue2_level::{Actor, MeshShape, Movement};

use super::Batch;
use super::camera;
use super::load::{Vertex, mesh_vertex};
use super::particle_mesh::ParticleMesh;
use super::pipeline::Draw;

pub(crate) struct Mover {
    mesh: ParticleMesh,
    actor: Actor,
    movement: Movement,
    /// Whether the hour lights it, in a world zone.
    lit: bool,
}

impl Mover {
    /// How many sections it draws, one batch each.
    pub(super) fn sections(&self) -> usize {
        self.mesh.sections.len()
    }

    /// `actor` as a mover, when it sways and its mesh loads; `lit` in world zones.
    pub(super) fn load(actor: &Actor, catalog: &mut Catalog, lit: bool) -> Option<Self> {
        let movement = actor.movement?;
        let path = actor.static_mesh.as_ref()?;
        let shape = MeshShape {
            mesh: path.clone(),
            materials: actor.skins.iter().cloned().map(Some).collect(),
            own_blend: true,
            scale: [[1.0; 2]; 3],
        };
        let mesh = ParticleMesh::load(catalog, &shape)?;
        Some(Self { mesh, actor: actor.clone(), movement, lit })
    }

    pub(super) fn materials(&self) -> impl Iterator<Item = &Material> {
        self.mesh.sections.iter().map(|(material, _)| material)
    }

    /// The mesh where it has swung to `time` seconds after load, relative to `camera`, onto `vertices`.
    #[expect(clippy::cast_possible_truncation, reason = "rotation offsets stay within a few turns")]
    pub(super) fn write(&self, time: f32, camera: [f32; 3], vertices: &mut Vec<Vertex>) {
        let Movement { origin, translation, rotation } = self.movement;
        let mut center = origin.location;
        for ((center, offset), camera) in center.iter_mut().zip(translation.offset(time)).zip(camera) {
            *center += offset - camera;
        }
        let mut turned = origin.rotation;
        for (turned, offset) in turned.iter_mut().zip(rotation.offset(time)) {
            *turned += offset.round() as i32;
        }
        let axes = camera::axes(turned);
        for (index, position, uv, normal) in self.mesh.corners() {
            let at = camera::place(position, self.actor.scale, &axes, center);
            let normal = camera::place(normal, self.actor.scale.map(f32::signum), &axes, [0.0; 3]);
            let index = u16::try_from(index).unwrap_or(u16::MAX);
            vertices.push(mesh_vertex(&self.actor, index, at, uv, 1.0, self.lit.then_some(normal)));
        }
    }
}

/// One batch per section of every mover, drawn as solid scene geometry, over vertices starting at `first` and
/// indices appended to `indices`; returns the batches and the vertices the movers write.
pub(super) fn layout(
    movers: &[Mover],
    first: usize,
    indices: &mut Vec<u32>,
    batch: &mut dyn FnMut(Material, Draw, bool, f32, Range<u32>) -> Option<Batch>,
) -> Result<(Vec<Batch>, usize), String> {
    let mut batches = Vec::new();
    let mut vertices = first;
    for mover in movers {
        let base = u32::try_from(vertices).map_err(|_| "too many mover vertices")?;
        for (material, section) in &mover.mesh.sections {
            let start = u32::try_from(indices.len()).map_err(|_| "too many mover indices")?;
            indices.extend(section.iter().map(|index| base + index));
            let end = u32::try_from(indices.len()).map_err(|_| "too many mover indices")?;
            batches.extend(batch(material.clone(), Draw::surface(material.blend), true, 0.0, start..end));
        }
        vertices += mover.mesh.len();
    }
    Ok((batches, vertices - first))
}

//! The meshes `MeshEmitter` particles draw: loaded with their materials once, then placed for every particle
//! each frame into the particle vertex buffer.

use l2_catalog::{Catalog, Material, Mesh};
use ue2_level::MeshShape;

use super::camera;
use super::load::{Vertex, vertex};

/// Emitters naming the same mesh with the same overrides share one loaded mesh.
pub(crate) type MeshKey = (String, Vec<Option<String>>);

pub(crate) fn key(shape: &MeshShape) -> MeshKey {
    (shape.mesh.clone(), shape.materials.clone())
}

/// A mesh ready to draw as particles: vertex streams, and each section's material with its indices.
pub(crate) struct ParticleMesh {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    normals: Vec<[f32; 3]>,
    pub(crate) sections: Vec<(Material, Vec<u32>)>,
}

impl ParticleMesh {
    /// The mesh `shape` names, with its override materials where given; sections whose material does not
    /// resolve are left out.
    pub(super) fn load(catalog: &mut Catalog, shape: &MeshShape) -> Option<Self> {
        let Mesh { mesh, materials } = catalog.static_mesh(&shape.mesh)?;
        let sections = mesh
            .sections
            .iter()
            .enumerate()
            .filter_map(|(slot, section)| {
                let path = shape.materials.get(slot).cloned().flatten().or_else(|| materials.get(slot).cloned()?)?;
                let material = catalog.material(&path)?;
                let first = section.first_index as usize;
                let indices = mesh.indices.get(first..first + section.triangles as usize * 3)?;
                Some((material, indices.iter().copied().map(u32::from).collect()))
            })
            .collect();
        Some(Self { positions: mesh.positions, uvs: mesh.uvs, normals: mesh.normals, sections })
    }

    /// Each vertex in the mesh's own space: its index, position, texture coordinates and normal.
    pub(super) fn corners(&self) -> impl Iterator<Item = (usize, [f32; 3], [f32; 2], [f32; 3])> + '_ {
        self.positions.iter().enumerate().map(|(index, &position)| {
            let uv = self.uvs.get(index).copied().unwrap_or_default();
            (index, position, uv, self.normals.get(index).copied().unwrap_or_default())
        })
    }

    /// Vertices one particle writes.
    pub(crate) fn len(&self) -> usize {
        self.positions.len()
    }

    /// The mesh scaled by `scale` on each axis, turned by `axes`, standing at `center` and colored `color`, onto
    /// `vertices`.
    // ponytail: particles take the emitter's rotation; their own spin is left out until a scene shows it.
    pub(super) fn write(
        &self,
        center: [f32; 3],
        scale: [f32; 3],
        axes: &[[f32; 3]; 3],
        color: [f32; 4],
        vertices: &mut Vec<Vertex>,
    ) {
        for (_, position, uv, _) in self.corners() {
            vertices.push(vertex(camera::place(position, scale, axes, center), uv, color));
        }
    }
}

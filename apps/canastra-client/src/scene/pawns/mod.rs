//! Characters standing in the scene: body parts skinned to a skeleton that plays an animation sequence,
//! reposed on the CPU every frame.
// ponytail: CPU skinning suits the few characters of the lobby; move it to the GPU when crowds need it.

mod skeleton;

use std::ops::Range;

use l2_catalog::{Catalog, Material};
use ue2_assets::{MeshAnimation, SkeletalMesh};

use super::camera;
use super::load::Vertex;
use skeleton::Transform;

/// A body part to load: its skeletal mesh and the texture it wears, as client paths.
pub(crate) struct PartSource {
    pub(crate) mesh: String,
    pub(crate) texture: String,
}

pub(crate) struct Pawn {
    pub(crate) parts: Vec<Part>,
    location: [f32; 3],
    axes: [[f32; 3]; 3],
}

pub(crate) struct Part {
    material: Material,
    mesh: SkeletalMesh,
    /// Where each bone stands in the bind pose, and each bone's bind pose relative to its parent.
    bind: Vec<Transform>,
    bind_locals: Vec<Transform>,
    animation: Option<Animation>,
    mesh_axes: [[f32; 3]; 3],
}

/// The sequence a part plays and the track of each of its bones.
struct Animation {
    sequence: ue2_assets::Sequence,
    tracks: Vec<Option<usize>>,
}

impl Pawn {
    /// A character at `location` facing `rotation`, playing `sequence`; parts that cannot be loaded are
    /// left out.
    pub(crate) fn load(
        catalog: &mut Catalog,
        sources: &[PartSource],
        location: [f32; 3],
        rotation: [i32; 3],
        sequence: &str,
    ) -> Self {
        // Parts share the body's skeleton, but only some name the animation it plays, hair among those that do
        // not; every part plays the first one found.
        let loaded: Vec<(SkeletalMesh, Option<String>, Material)> = sources
            .iter()
            .filter_map(|source| {
                let (mesh, animation) = catalog.skeletal_mesh(&source.mesh)?;
                Some((mesh, animation, catalog.material(&source.texture)?))
            })
            .collect();
        let animation =
            loaded.iter().find_map(|(_, path, _)| path.as_deref()).and_then(|path| catalog.mesh_animation(path));
        let parts = loaded
            .into_iter()
            .map(|(mesh, _, material)| Part::new(mesh, material, animation.as_ref(), sequence))
            .collect();
        Self { parts, location, axes: camera::axes(rotation) }
    }

    /// Writes every part's vertices at scene time `time`, relative to `camera`, part after part.
    pub(crate) fn write(&self, time: f32, camera: [f32; 3], out: &mut Vec<Vertex>) {
        for part in &self.parts {
            let pose = part.pose(time);
            for vertex in &part.mesh.vertices {
                let skinned = skeleton::skin(vertex, &part.bind, &pose);
                let turned = camera::place(skinned, part.mesh.scale, &part.mesh_axes, [0.0; 3]);
                let mut at = camera::place(turned, [1.0; 3], &self.axes, self.location);
                for (at, camera) in at.iter_mut().zip(camera) {
                    *at -= camera;
                }
                out.push([at[0], at[1], at[2], vertex.uv[0], vertex.uv[1], 1.0, 1.0, 1.0, 1.0]);
            }
        }
    }
}

/// The pawns' parts laid out one after another, as `Pawn::write` fills their vertices.
pub(crate) struct Layout {
    /// Triangle indices of all parts.
    pub(crate) indices: Vec<u32>,
    /// Each part's material and its range of `indices`.
    pub(crate) ranges: Vec<(Material, Range<u32>)>,
    pub(crate) vertices: usize,
}

pub(crate) fn layout(pawns: &[Pawn]) -> Layout {
    let (mut indices, mut ranges, mut vertices) = (Vec::new(), Vec::new(), 0);
    for part in pawns.iter().flat_map(|pawn| &pawn.parts) {
        let base = u32::try_from(vertices).unwrap_or(u32::MAX);
        let start = u32::try_from(indices.len()).unwrap_or(u32::MAX);
        indices.extend(part.mesh.indices.iter().map(|&index| base.saturating_add(u32::from(index))));
        ranges.push((part.material.clone(), start..u32::try_from(indices.len()).unwrap_or(u32::MAX)));
        vertices += part.mesh.vertices.len();
    }
    Layout { indices, ranges, vertices }
}

impl Part {
    fn new(mesh: SkeletalMesh, material: Material, animation: Option<&MeshAnimation>, sequence: &str) -> Self {
        let bind_locals = skeleton::bind_locals(&mesh.bones);
        let bind = skeleton::world(&bind_locals, mesh.bones.iter().map(|bone| bone.parent));
        let animation = animation.and_then(|animation| Animation::of(&mesh, animation, sequence));
        let mesh_axes = camera::axes(mesh.rotation);
        Self { material, mesh, bind, bind_locals, animation, mesh_axes }
    }

    /// Where each bone stands at scene time `time`, looping the sequence.
    fn pose(&self, time: f32) -> Vec<Transform> {
        let Some(animation) = &self.animation else { return self.bind.clone() };
        let frames = animation.sequence.frames.max(1) as f32;
        let frame = (time * animation.sequence.rate).rem_euclid(frames);
        let locals = skeleton::sequence_locals(&animation.sequence, &animation.tracks, &self.bind_locals, frame);
        skeleton::world(&locals, self.mesh.bones.iter().map(|bone| bone.parent))
    }
}

impl Animation {
    /// `sequence` of `animation`, with its tracks matched to the mesh's bones by name.
    fn of(mesh: &SkeletalMesh, animation: &MeshAnimation, sequence: &str) -> Option<Self> {
        let sequence = animation.sequences.iter().find(|found| found.name.eq_ignore_ascii_case(sequence))?.clone();
        let tracks = mesh
            .bones
            .iter()
            .map(|bone| {
                let index = animation.bones.iter().position(|named| named.name.eq_ignore_ascii_case(&bone.name))?;
                sequence.tracks.iter().position(|track| track.bone == index)
            })
            .collect();
        Some(Self { sequence, tracks })
    }
}

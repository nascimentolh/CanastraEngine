//! Characters standing in the scene: body parts skinned to a skeleton that plays an animation sequence,
//! reposed on the CPU every frame.
// ponytail: CPU skinning suits the few characters of the lobby; move it to the GPU when crowds need it.

mod skeleton;

use std::ops::Range;

use l2_catalog::{Catalog, Material, Skinned};
use ue2_assets::{MeshAnimation, SkeletalMesh, SkinVertex};

use super::camera;
use super::daylight::Daylight;
use super::load::Vertex;
use skeleton::Transform;

/// A character to stand in the scene, as client paths.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Figure {
    pub(crate) parts: Vec<PartSource>,
    pub(crate) location: [f32; 3],
    /// Unreal rotation units.
    pub(crate) yaw: i32,
    /// The sequence to loop, without the body's suffix, e.g. `Wait_Hand`.
    pub(crate) sequence: &'static str,
}

/// A body part: its skeletal mesh and the texture of each section; sections without one keep the mesh's.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PartSource {
    pub(crate) mesh: String,
    pub(crate) textures: Vec<String>,
}

pub(crate) struct Pawn {
    parts: Vec<Part>,
    location: [f32; 3],
    axes: [[f32; 3]; 3],
}

struct Part {
    mesh: SkeletalMesh,
    /// Each section's material and its range of the mesh's indices.
    sections: Vec<(Material, Range<usize>)>,
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
    /// The character `figure` describes; parts and sections that cannot be loaded are left out.
    pub(crate) fn load(catalog: &mut Catalog, figure: &Figure) -> Self {
        let loaded: Vec<_> = figure
            .parts
            .iter()
            .filter_map(|source| {
                let skinned = catalog.skeletal_mesh(&source.mesh)?;
                let sections = sections(catalog, &skinned, &source.textures);
                Some((skinned, sections))
            })
            .collect();
        // Parts share the body's skeleton, but only some name the animation it plays, hair among those that do
        // not; every part plays the first one found, whose name ends each sequence's name.
        let animation_path = loaded.iter().find_map(|(skinned, _)| skinned.animation.clone());
        let animation = animation_path.as_deref().and_then(|path| catalog.mesh_animation(path));
        let suffix = animation_path.as_deref().and_then(|path| path.rsplit('.').next()?.strip_suffix("_anim"));
        let sequence = format!("{}_{}", figure.sequence, suffix.unwrap_or_default());
        let parts = loaded
            .into_iter()
            .map(|(skinned, sections)| Part::new(skinned.mesh, sections, animation.as_ref(), &sequence))
            .collect();
        Self { parts, location: figure.location, axes: camera::axes([0, figure.yaw, 0]) }
    }

    /// Writes every part's vertices at scene time `time`, relative to `camera` and lit by `daylight` if given,
    /// part after part.
    pub(crate) fn write(&self, time: f32, camera: [f32; 3], daylight: Option<&Daylight>, out: &mut Vec<Vertex>) {
        for part in &self.parts {
            let pose = part.pose(time);
            for vertex in &part.mesh.vertices {
                let skinned = skeleton::skin(vertex, &part.bind, &pose);
                let turned = camera::place(skinned, part.mesh.scale, &part.mesh_axes, [0.0; 3]);
                let mut at = camera::place(turned, [1.0; 3], &self.axes, self.location);
                for (at, camera) in at.iter_mut().zip(camera) {
                    *at -= camera;
                }
                let [r, g, b] = daylight.map_or([1.0; 3], |daylight| {
                    // The normal skins as the offset between the vertex and a point one unit along it.
                    let tip = skeleton::skin(
                        &SkinVertex { position: skeleton::add(vertex.position, vertex.normal), ..*vertex },
                        &part.bind,
                        &pose,
                    );
                    let turned = camera::place(sub(tip, skinned), part.mesh.scale, &part.mesh_axes, [0.0; 3]);
                    daylight.on_shaded(camera::place(turned, [1.0; 3], &self.axes, [0.0; 3]), 1.0)
                });
                out.push([at[0], at[1], at[2], vertex.uv[0], vertex.uv[1], r, g, b, 1.0]);
            }
        }
    }
}

/// Each section's material, from `textures` where given and the mesh's own otherwise, with its indices.
fn sections(catalog: &mut Catalog, skinned: &Skinned, textures: &[String]) -> Vec<(Material, Range<usize>)> {
    skinned
        .mesh
        .sections
        .iter()
        .enumerate()
        .filter_map(|(index, section)| {
            let path = textures.get(index).or(skinned.materials.get(index)?.as_ref())?;
            let first = section.first_index as usize;
            Some((catalog.material(path)?, first..first + section.triangles as usize * 3))
        })
        .collect()
}

/// The pawns' parts laid out one after another, as `Pawn::write` fills their vertices.
pub(crate) struct Layout {
    /// Triangle indices of all parts.
    pub(crate) indices: Vec<u32>,
    /// Each section's material and its range of `indices`.
    pub(crate) ranges: Vec<(Material, Range<u32>)>,
    pub(crate) vertices: usize,
}

pub(crate) fn layout(pawns: &[Pawn]) -> Layout {
    let (mut indices, mut ranges, mut vertices) = (Vec::new(), Vec::new(), 0);
    for part in pawns.iter().flat_map(|pawn| &pawn.parts) {
        let base = u32::try_from(vertices).unwrap_or(u32::MAX);
        for (material, range) in &part.sections {
            let start = u32::try_from(indices.len()).unwrap_or(u32::MAX);
            let section = part.mesh.indices.get(range.clone()).unwrap_or_default();
            indices.extend(section.iter().map(|&index| base.saturating_add(u32::from(index))));
            ranges.push((material.clone(), start..u32::try_from(indices.len()).unwrap_or(u32::MAX)));
        }
        vertices += part.mesh.vertices.len();
    }
    Layout { indices, ranges, vertices }
}

impl Part {
    fn new(
        mesh: SkeletalMesh,
        sections: Vec<(Material, Range<usize>)>,
        animation: Option<&MeshAnimation>,
        sequence: &str,
    ) -> Self {
        let bind_locals = skeleton::bind_locals(&mesh.bones);
        let bind = skeleton::world(&bind_locals, mesh.bones.iter().map(|bone| bone.parent));
        let animation = animation.and_then(|animation| Animation::of(&mesh, animation, sequence));
        let mesh_axes = camera::axes(mesh.rotation);
        Self { mesh, sections, bind, bind_locals, animation, mesh_axes }
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

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

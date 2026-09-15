//! Characters standing in the scene: body parts skinned to a skeleton that plays an animation sequence,
//! reposed on the CPU every frame.
// ponytail: CPU skinning suits the few characters of the lobby; move it to the GPU when crowds need it.

mod head;
mod held;
mod skeleton;

use std::ops::Range;

use l2_catalog::{Catalog, Material, Skinned};
use ue2_assets::{MeshAnimation, SkeletalMesh, SkinVertex};

use super::camera;
use super::daylight::Daylight;
use super::load::Vertex;
use held::Held;
pub(crate) use held::HeldSource;
use skeleton::Transform;

/// A character to stand in the scene, as client paths.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Figure {
    pub(crate) parts: Vec<PartSource>,
    pub(crate) held: Vec<HeldSource>,
    pub(crate) location: [f32; 3],
    /// Unreal rotation units.
    pub(crate) yaw: i32,
    /// The sequence to loop, without the body's suffix, e.g. `Wait_Hand`.
    pub(crate) sequence: &'static str,
    /// Text shown above the head, such as the character's name.
    pub(crate) label: Option<String>,
}

/// A body part: its skeletal mesh and the texture of each section; sections without one keep the mesh's.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PartSource {
    pub(crate) mesh: String,
    pub(crate) textures: Vec<String>,
    /// For a part with a skeleton of its own, the body bone its root hangs from, e.g. back hair from the head.
    pub(crate) follow: Option<&'static str>,
}

/// How far above the top of the head a label stands, in world units.
const LABEL_LIFT: f32 = 6.0;

pub(crate) struct Pawn {
    parts: Vec<Part>,
    /// The part with the whole body's skeleton, whose bones place those the other parts share with it.
    body: usize,
    held: Vec<Held>,
    label: Option<String>,
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
    /// For each bone, the same bone in the body's skeleton; empty for the body itself.
    in_body: Vec<Option<usize>>,
    /// The body bone this part's root hangs from, by index in the body's skeleton.
    follow: Option<usize>,
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
                Some((skinned, sections, source.follow))
            })
            .collect();
        // Body parts share the body's skeleton, but only some name its `<Body>_anim`, hair among those that do not;
        // they all play it, and its name ends each sequence's name. Parts with a skeleton of their own, such as
        // Kamael wings, name their own animation, with sequences of the same names.
        let animation_path = loaded
            .iter()
            .filter_map(|(skinned, _, _)| skinned.animation.clone())
            .find(|path| path.to_ascii_lowercase().ends_with("_anim"));
        let animation = animation_path.as_deref().and_then(|path| catalog.mesh_animation(path));
        let suffix = animation_path.as_deref().and_then(|path| path.rsplit('.').next()?.strip_suffix("_anim"));
        let sequence = format!("{}_{}", figure.sequence, suffix.unwrap_or_default());
        if animation.as_ref().is_some_and(|animation| {
            !animation.sequences.iter().any(|found| found.name.eq_ignore_ascii_case(&sequence))
        }) {
            eprintln!("pawn: no sequence {sequence} in {}", animation_path.as_deref().unwrap_or_default());
        }
        // The body is the part naming the body's animation, such as the face; every part is drawn with its scale and
        // rotation, as parts share the body's instance: some armor meshes store neither.
        let body = loaded
            .iter()
            .position(|(skinned, _, _)| skinned.animation.is_some() && skinned.animation == animation_path);
        let follows: Vec<_> = loaded.iter().map(|(_, _, follow)| *follow).collect();
        let mut parts: Vec<Part> = loaded
            .into_iter()
            .map(|(skinned, sections, _)| {
                let own = skinned.animation.as_deref().filter(|path| Some(*path) != animation_path.as_deref());
                let own = own.and_then(|path| catalog.mesh_animation(path));
                Part::new(skinned.mesh, sections, own.as_ref().or(animation.as_ref()), &sequence)
            })
            .collect();
        // Bones a part shares with the body follow the body's, or their local keys would turn about the wrong parents.
        let body = body.unwrap_or(0);
        let body_bones = parts.get(body).map(|part| part.mesh.bones.clone()).unwrap_or_default();
        for ((_, part), follow) in parts.iter_mut().enumerate().zip(follows).filter(|((index, _), _)| *index != body) {
            part.follow = follow.and_then(|name| head::find(&body_bones, name));
            part.in_body = part.mesh.bones.iter().map(|bone| head::find(&body_bones, &bone.name)).collect();
        }
        let held = figure.held.iter().filter_map(|source| Held::load(catalog, source, &parts)).collect();
        let label = figure.label.clone();
        Self { parts, body, held, label, location: figure.location, axes: camera::axes([0, figure.yaw, 0]) }
    }

    /// Where the pawn stands on the ground, relative to `camera`.
    pub(crate) fn feet(&self, camera: [f32; 3]) -> [f32; 3] {
        sub(self.location, camera)
    }

    /// The pawn's label and where it stands at scene time `time`, above the head, relative to `camera`.
    pub(crate) fn label(&self, time: f32, camera: [f32; 3]) -> Option<(&str, [f32; 3])> {
        let label = self.label.as_deref()?;
        let part = self.parts.get(self.body)?;
        let bone = head::find(&part.mesh.bones, "Bip01_HeadNub")?;
        let head = part.pose(time, &[], &[]).get(bone)?.translation;
        let turned = camera::place(head, part.mesh.scale, &part.mesh_axes, [0.0; 3]);
        let mut at = camera::place(turned, [1.0; 3], &self.axes, self.location);
        at[2] += LABEL_LIFT;
        Some((label, [at[0] - camera[0], at[1] - camera[1], at[2] - camera[2]]))
    }

    /// Writes the vertices of every part, then of everything held, at scene time `time`, relative to `camera`
    /// and lit by `daylight` if given.
    pub(crate) fn write(&self, time: f32, camera: [f32; 3], daylight: Option<&Daylight>, out: &mut Vec<Vertex>) {
        let body_part = self.parts.get(self.body);
        let body = body_part.map(|part| part.pose(time, &[], &[])).unwrap_or_default();
        let body_bind = body_part.map_or(&[][..], |part| &part.bind);
        let poses: Vec<Vec<Transform>> = self
            .parts
            .iter()
            .enumerate()
            .map(|(index, part)| if index == self.body { body.clone() } else { part.pose(time, &body, body_bind) })
            .collect();
        let Some(body_part) = body_part else { return };
        let (scale, mesh_axes) = (body_part.mesh.scale, body_part.mesh_axes);
        // Places a vertex given in the space of the skeleton.
        let mut push = |position: [f32; 3], normal: [f32; 3], uv: [f32; 2]| {
            let turned = camera::place(position, scale, &mesh_axes, [0.0; 3]);
            let mut at = camera::place(turned, [1.0; 3], &self.axes, self.location);
            for (at, camera) in at.iter_mut().zip(camera) {
                *at -= camera;
            }
            let [r, g, b] = daylight.map_or([1.0; 3], |daylight| {
                let turned = camera::place(normal, scale, &mesh_axes, [0.0; 3]);
                daylight.on_shaded(camera::place(turned, [1.0; 3], &self.axes, [0.0; 3]), 1.0)
            });
            out.push([at[0], at[1], at[2], uv[0], uv[1], r, g, b, 1.0]);
        };
        for (part, pose) in self.parts.iter().zip(&poses) {
            for vertex in &part.mesh.vertices {
                let skinned = skeleton::skin(vertex, &part.bind, pose);
                // The normal skins as the offset between the vertex and a point one unit along it.
                let tip = SkinVertex { position: skeleton::add(vertex.position, vertex.normal), ..*vertex };
                push(skinned, sub(skeleton::skin(&tip, &part.bind, pose), skinned), vertex.uv);
            }
        }
        for held in &self.held {
            let Some(&bone) = poses.get(held.part).and_then(|pose| pose.get(held.bone)) else { continue };
            for (position, normal, uv) in held.vertices(bone) {
                push(position, normal, uv);
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
    for pawn in pawns {
        let parts = pawn.parts.iter().map(|part| (&part.mesh.indices, &part.sections, part.mesh.vertices.len()));
        let held = pawn.held.iter().map(|held| (&held.mesh.indices, &held.sections, held.mesh.vertices.len()));
        for (mesh_indices, sections, count) in parts.chain(held) {
            let base = u32::try_from(vertices).unwrap_or(u32::MAX);
            for (material, range) in sections {
                let start = u32::try_from(indices.len()).unwrap_or(u32::MAX);
                let section = mesh_indices.get(range.clone()).unwrap_or_default();
                indices.extend(section.iter().map(|&index| base.saturating_add(u32::from(index))));
                ranges.push((material.clone(), start..u32::try_from(indices.len()).unwrap_or(u32::MAX)));
            }
            vertices += count;
        }
    }
    Layout { indices, ranges, vertices }
}

impl Part {
    fn new(
        mut mesh: SkeletalMesh,
        sections: Vec<(Material, Range<usize>)>,
        animation: Option<&MeshAnimation>,
        sequence: &str,
    ) -> Self {
        head::carry(&mesh.bones, &mut mesh.vertices);
        let bind_locals = skeleton::bind_locals(&mesh.bones);
        let bind = skeleton::world(&bind_locals, mesh.bones.iter().map(|bone| bone.parent), |_| None);
        let animation = animation.and_then(|animation| Animation::of(&mesh, animation, sequence));
        let mesh_axes = camera::axes(mesh.rotation);
        Self { mesh, sections, bind, bind_locals, animation, mesh_axes, in_body: Vec::new(), follow: None }
    }

    /// Where each bone stands at scene time `time`, looping the sequence; bones shared with the body stand where
    /// `body`, the body's pose, puts them, and a following part's root moves with its body bone from `body_bind`.
    fn pose(&self, time: f32, body: &[Transform], body_bind: &[Transform]) -> Vec<Transform> {
        let locals = match &self.animation {
            Some(animation) => {
                let frames = animation.sequence.frames.max(1) as f32;
                let frame = (time * animation.sequence.rate).rem_euclid(frames);
                skeleton::sequence_locals(&animation.sequence, &animation.tracks, &self.bind_locals, frame)
            }
            None => self.bind_locals.clone(),
        };
        let followed = self.follow.and_then(|bone| {
            let moved = body.get(bone)?.then(body_bind.get(bone)?.inverse());
            Some(moved.then(*self.bind.first()?))
        });
        let known = |index: usize| match (index, followed) {
            (0, Some(root)) => Some(root),
            _ => self.in_body.get(index).copied().flatten().and_then(|bone| body.get(bone)).copied(),
        };
        skeleton::world(&locals, self.mesh.bones.iter().map(|bone| bone.parent), known)
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
                let index = animation.bones.iter().position(|named| head::same(&named.name, &bone.name))?;
                sequence.tracks.iter().position(|track| track.bone == index)
            })
            .collect();
        Some(Self { sequence, tracks })
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

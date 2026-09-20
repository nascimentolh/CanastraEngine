//! What a character holds, such as weapons and shields: one-bone meshes that follow the bone they hang from.

use std::ops::Range;

use l2_catalog::{Catalog, Material};
use ue2_assets::SkeletalMesh;

use super::skeleton::Transform;
use super::{Part, head, sections};

/// A held mesh as client paths, and the bone of the body's skeleton it hangs from, e.g. `Weapon_R_Bone`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HeldSource {
    pub(crate) mesh: String,
    pub(crate) textures: Vec<String>,
    pub(crate) bone: &'static str,
}

pub(super) struct Held {
    pub(super) mesh: SkeletalMesh,
    pub(super) sections: Vec<(Material, Range<usize>)>,
    /// The part whose skeleton has the bone, and the bone's index in it.
    pub(super) part: usize,
    pub(super) bone: usize,
}

impl Held {
    /// `source` on the first of `parts` with its bone; `None` when the mesh or the bone is missing.
    pub(super) fn load(catalog: &mut Catalog, source: &HeldSource, parts: &[Part]) -> Option<Self> {
        let (part, bone) = parts.iter().enumerate().find_map(|(index, part)| {
            let bone = head::find(&part.mesh.bones, source.bone)?;
            Some((index, bone))
        })?;
        let skinned = catalog.skeletal_mesh(&source.mesh)?;
        let sections = sections(catalog, &skinned, &source.textures);
        Some(Self { mesh: skinned.mesh, sections, part, bone })
    }

    /// Each vertex's position, normal and UV in the space of the skeleton, hung from `bone` as it stands.
    ///
    /// A held mesh sits in the bone's frame with no offset, its own root bone aside.
    // ponytail: the mesh's own scale and rotation are taken as identity, as the lobby's weapons store them; the
    // orientation still differs from H5 for some weapons.
    pub(super) fn vertices(&self, bone: Transform) -> impl Iterator<Item = ([f32; 3], [f32; 3], [f32; 2])> + '_ {
        self.mesh
            .vertices
            .iter()
            .map(move |vertex| (bone.apply(vertex.position), bone.rotate(vertex.normal), vertex.uv))
    }
}

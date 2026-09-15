//! The head and the rigid parts it carries, such as the face and hair.

use ue2_assets::{Bone, SkinVertex};

/// The body's head bone (`Pawn.HeadBone`).
const BONE: &str = "Bip01_Head";

/// The index of the bone named `name`; biped names come with spaces or underscores, e.g. `Bip01 Head`.
pub(super) fn find(bones: &[Bone], name: &str) -> Option<usize> {
    bones.iter().position(|bone| same(&bone.name, name))
}

/// Whether two bone names are the same, whatever their case and whether they separate words with spaces.
pub(super) fn same(a: &str, b: &str) -> bool {
    let letter = |a: u8, b: u8| a.eq_ignore_ascii_case(&b) || matches!((a, b), (b' ', b'_') | (b'_', b' '));
    a.len() == b.len() && a.bytes().zip(b.bytes()).all(|(a, b)| letter(a, b))
}

/// Hangs a mesh bound whole to its root from the head instead. Faces and hair are rigid on the root bone but
/// modeled around the head of the bind pose, and the client carries them with the head.
pub(super) fn carry(bones: &[Bone], vertices: &mut [SkinVertex]) {
    let rigid = vertices.iter().all(|vertex| vertex.bones == [0; 4]);
    let Some(head) = find(bones, BONE).and_then(|head| u16::try_from(head).ok()) else { return };
    if rigid && head != 0 {
        for vertex in vertices {
            vertex.bones[0] = head;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_rigid_on_the_root_hangs_from_the_head_whatever_its_name_is_spelled() {
        let bone =
            |name: &str| Bone { name: name.into(), parent: 0, rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3] };
        let vertex = SkinVertex {
            position: [0.0; 3],
            normal: [0.0; 3],
            uv: [0.0; 2],
            bones: [0; 4],
            weights: [1.0, 0.0, 0.0, 0.0],
        };
        let mut hair = [vertex];
        carry(&[bone("Bip01"), bone("Bip01 Pelvis"), bone("bip01 head")], &mut hair);
        assert_eq!(hair[0].bones, [2, 0, 0, 0]);

        let mut sleeve = [SkinVertex { bones: [1, 0, 0, 0], ..vertex }, vertex];
        carry(&[bone("Bip01"), bone("Bip01_Pelvis"), bone("Bip01_Head")], &mut sleeve);
        assert_eq!(sleeve[1].bones, [0; 4]);
    }
}

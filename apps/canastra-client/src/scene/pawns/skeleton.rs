//! Bone poses and skinning: a sequence's keys at a moment, turned into where each bone stands, and body part
//! vertices moved with the bones they are weighted to.

use ue2_assets::{Bone, Sequence, SkinVertex};

/// A rotation, as a unit quaternion (x, y, z, w), followed by a translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Transform {
    pub(super) rotation: [f32; 4],
    pub(super) translation: [f32; 3],
}

impl Transform {
    const IDENTITY: Self = Self { rotation: [0.0, 0.0, 0.0, 1.0], translation: [0.0; 3] };

    /// `self` applied after `local`: a child's transform placed by its parent's.
    fn then(self, local: Self) -> Self {
        Self {
            rotation: multiply(self.rotation, local.rotation),
            translation: add(self.translation, rotate(self.rotation, local.translation)),
        }
    }

    pub(super) fn apply(self, point: [f32; 3]) -> [f32; 3] {
        add(rotate(self.rotation, point), self.translation)
    }

    pub(super) fn rotate(self, direction: [f32; 3]) -> [f32; 3] {
        rotate(self.rotation, direction)
    }

    fn inverse(self) -> Self {
        let [x, y, z, w] = self.rotation;
        let rotation = [-x, -y, -z, w];
        Self { rotation, translation: rotate(rotation, self.translation.map(|axis| -axis)) }
    }
}

/// Where each bone stands in the space of the mesh, from each bone's rotation and translation relative to
/// its parent. Parents come before their children.
pub(super) fn world(locals: &[Transform], parents: impl Iterator<Item = usize>) -> Vec<Transform> {
    let mut world: Vec<Transform> = Vec::with_capacity(locals.len());
    for (index, (local, parent)) in locals.iter().zip(parents).enumerate() {
        let placed = if parent < index {
            world.get(parent).copied().unwrap_or(Transform::IDENTITY)
        } else {
            Transform::IDENTITY
        };
        world.push(placed.then(*local));
    }
    world
}

/// Each bone's bind pose relative to its parent.
pub(super) fn bind_locals(bones: &[Bone]) -> Vec<Transform> {
    bones
        .iter()
        .enumerate()
        .map(|(index, bone)| {
            let rotation = stored_rotation(index, bone.rotation);
            Transform { rotation, translation: bone.position }
        })
        .collect()
}

/// Each bone's pose relative to its parent at `frame`, for a mesh whose bones map to the sequence's tracks
/// through `tracks` (the track of each mesh bone, if any). Bones without a track keep their bind pose.
pub(super) fn sequence_locals(
    sequence: &Sequence,
    tracks: &[Option<usize>],
    bind: &[Transform],
    frame: f32,
) -> Vec<Transform> {
    bind.iter()
        .zip(tracks)
        .enumerate()
        .map(|(index, (&bind, track))| {
            let Some(track) = track.and_then(|track| sequence.tracks.get(track)) else { return bind };
            // The root keeps its bind rotation: its keys turn female idles away from where the pawn faces, and
            // H5 stands every lobby character facing its yaw.
            let rotation = match index {
                0 => bind.rotation,
                _ => sample(&track.rotations, &track.times, frame, slerp)
                    .map_or(bind.rotation, |q| stored_rotation(index, q)),
            };
            let translation = sample(&track.positions, &track.times, frame, lerp).unwrap_or(bind.translation);
            Transform { rotation, translation }
        })
        .collect()
}

/// A skinned vertex's position: its bind position moved by each bone it is weighted to, from `bind` to `pose`.
pub(super) fn skin(vertex: &SkinVertex, bind: &[Transform], pose: &[Transform]) -> [f32; 3] {
    let mut out = [0.0; 3];
    let mut total = 0.0;
    for (&bone, &weight) in vertex.bones.iter().zip(&vertex.weights) {
        let (Some(&bind), Some(&pose)) = (bind.get(usize::from(bone)), pose.get(usize::from(bone))) else { continue };
        if weight <= 0.0 {
            continue;
        }
        let moved = pose.apply(bind.inverse().apply(vertex.position));
        for (out, moved) in out.iter_mut().zip(moved) {
            *out += moved * weight;
        }
        total += weight;
    }
    if total > 0.0 { out.map(|axis| axis / total) } else { vertex.position }
}

/// The value of keys at `frame`, between the keys around it; a single key holds throughout.
fn sample<T: Copy>(keys: &[T], times: &[f32], frame: f32, mix: fn(T, T, f32) -> T) -> Option<T> {
    let first = *keys.first()?;
    if keys.len() == 1 || times.len() < keys.len() {
        return Some(first);
    }
    let next = times.iter().position(|&time| time > frame).unwrap_or(keys.len());
    match next {
        0 => Some(first),
        next if next >= keys.len() => keys.last().copied(),
        next => {
            let (from, to) = (times.get(next - 1)?, times.get(next)?);
            let t = (frame - from) / (to - from).max(f32::EPSILON);
            Some(mix(*keys.get(next - 1)?, *keys.get(next)?, t))
        }
    }
}

fn lerp(from: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t, from[2] + (to[2] - from[2]) * t]
}

/// Normalized linear interpolation along the shorter arc, close to slerp for nearby keys.
fn slerp(from: [f32; 4], to: [f32; 4], t: f32) -> [f32; 4] {
    let dot: f32 = from.iter().zip(to).map(|(a, b)| a * b).sum();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    let mut mixed = from;
    for (axis, to) in mixed.iter_mut().zip(to) {
        *axis += (to * sign - *axis) * t;
    }
    let length = mixed.iter().map(|axis| axis * axis).sum::<f32>().sqrt().max(f32::EPSILON);
    mixed.map(|axis| axis / length)
}

fn multiply([ax, ay, az, aw]: [f32; 4], [bx, by, bz, bw]: [f32; 4]) -> [f32; 4] {
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

fn rotate([x, y, z, w]: [f32; 4], point: [f32; 3]) -> [f32; 3] {
    let axis = [x, y, z];
    let twice = cross(axis, point).map(|value| value * 2.0);
    add(add(point, twice.map(|value| value * w)), cross(axis, twice))
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub(super) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A stored bone rotation as the engine uses it. Unreal Engine 2 keeps every bone's rotation conjugated
/// except the root's.
fn stored_rotation(index: usize, [x, y, z, w]: [f32; 4]) -> [f32; 4] {
    if index == 0 { [x, y, z, w] } else { [-x, -y, -z, w] }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4)
    }

    #[test]
    fn the_bind_pose_keeps_vertices_and_a_turned_parent_carries_its_child() {
        let bones = [
            Bone { name: "root".into(), parent: 0, rotation: [0.0, 0.0, 0.0, 1.0], position: [0.0; 3] },
            Bone { name: "arm".into(), parent: 0, rotation: [0.0, 0.0, 0.0, 1.0], position: [10.0, 0.0, 0.0] },
        ];
        let locals = bind_locals(&bones);
        let bind = world(&locals, bones.iter().map(|bone| bone.parent));
        let vertex = SkinVertex {
            position: [12.0, 0.0, 0.0],
            normal: [0.0; 3],
            uv: [0.0; 2],
            bones: [1, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        };
        assert!(close(skin(&vertex, &bind, &bind), [12.0, 0.0, 0.0]));

        // A quarter turn of the root about Z swings the arm, and the vertex on it, from +X to +Y.
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let turned = [Transform { rotation: [0.0, 0.0, half, half], translation: [0.0; 3] }, locals[1]];
        let pose = world(&turned, bones.iter().map(|bone| bone.parent));
        assert!(close(skin(&vertex, &bind, &pose), [0.0, 12.0, 0.0]));
    }
}

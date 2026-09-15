//! Unreal's left-handed world (X forward, Y right, Z up) seen through a camera: rotations from
//! rotators and a reverse-Z perspective with no far plane.

/// Column-major, as WGSL reads it.
pub(crate) type Matrix = [[f32; 4]; 4];

/// Closest distance drawn, in world units.
const NEAR: f32 = 10.0;

/// Forward, right and up axes of a rotator (pitch, yaw, roll in 65536ths of a turn), as Unreal's
/// `FRotationMatrix` builds them.
pub(crate) fn axes([pitch, yaw, roll]: [i32; 3]) -> [[f32; 3]; 3] {
    let angle = |units: i32| f64::from(units) * std::f64::consts::TAU / 65536.0;
    let (sp, cp) = (angle(pitch).sin(), angle(pitch).cos());
    let (sy, cy) = (angle(yaw).sin(), angle(yaw).cos());
    let (sr, cr) = (angle(roll).sin(), angle(roll).cos());
    [
        [cp * cy, cp * sy, sp],
        [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp],
        [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp],
    ]
    .map(|axis| axis.map(|value| value as f32))
}

/// Places a local vertex: scaled on its own axes, rotated, then moved to `location`.
pub(crate) fn place(vertex: [f32; 3], scale: [f32; 3], axes: &[[f32; 3]; 3], location: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = [vertex[0] * scale[0], vertex[1] * scale[1], vertex[2] * scale[2]];
    let mut world = location;
    for (((world, forward), right), up) in world.iter_mut().zip(axes[0]).zip(axes[1]).zip(axes[2]) {
        *world += x * forward + y * right + z * up;
    }
    world
}

/// Projects points given relative to the camera. `fov` is horizontal, in degrees, as in Unreal.
pub(crate) fn view_projection(rotation: [i32; 3], fov: f32, aspect: f32) -> Matrix {
    let [forward, right, up] = axes(rotation);
    let x_scale = 1.0 / (fov.to_radians() / 2.0).tan();
    let y_scale = x_scale * aspect;
    // Columns of world x, y, z and w; within each, the rows are clip x, clip y, clip z (NEAR, so depth is
    // NEAR / distance) and clip w (the distance along the view).
    [
        [right[0] * x_scale, up[0] * y_scale, 0.0, forward[0]],
        [right[1] * x_scale, up[1] * y_scale, 0.0, forward[1]],
        [right[2] * x_scale, up[2] * y_scale, 0.0, forward[2]],
        [0.0, 0.0, NEAR, 0.0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(matrix: &Matrix, [x, y, z]: [f32; 3]) -> [f32; 3] {
        let clip =
            [0, 1, 2, 3].map(|row| matrix[0][row] * x + matrix[1][row] * y + matrix[2][row] * z + matrix[3][row]);
        [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]]
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4)
    }

    #[test]
    fn looks_along_the_rotation_with_right_on_screen_right() {
        // Yaw a quarter turn: looking down +Y, so +X is to the left and +Z up.
        let matrix = view_projection([0, 16384, 0], 90.0, 1.0);
        assert!(close(project(&matrix, [0.0, 100.0, 0.0]), [0.0, 0.0, 0.1]));
        assert!(close(project(&matrix, [-100.0, 100.0, 0.0]), [1.0, 0.0, 0.1]));
        assert!(close(project(&matrix, [0.0, 100.0, 100.0]), [0.0, 1.0, 0.1]));
        assert!(close(project(&matrix, [0.0, 1000.0, 0.0]), [0.0, 0.0, 0.01]));
    }

    #[test]
    fn places_vertices_scaled_rotated_and_moved() {
        let axes = axes([0, 16384, 0]);
        assert!(close(place([1.0, 0.0, 2.0], [2.0, 1.0, 3.0], &axes, [10.0, 0.0, 0.0]), [10.0, 2.0, 6.0]));
    }
}

//! Unreal's left-handed world (X forward, Y right, Z up) seen through a camera: rotations from
//! rotators and a reverse-Z perspective with no far plane.

/// Column-major, as WGSL reads it.
pub(crate) type Matrix = [[f32; 4]; 4];

/// Closest distance drawn, in world units.
pub(crate) const NEAR: f32 = 10.0;

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

/// The sides of what the camera sees, as planes in the space the scene's vertices are given in. A point is
/// inside the screen when it is on the positive side of every one of them.
pub(crate) fn sides(matrix: &Matrix) -> [[f32; 4]; 5] {
    let [[xx, xy, _, xw], [yx, yy, _, yw], [zx, zy, _, zw], [wx, wy, _, ww]] = *matrix;
    let (clip_x, clip_y) = ([xx, yx, zx, wx], [xy, yy, zy, wy]);
    let along = [xw, yw, zw, ww];
    let edge = |[ax, ay, az, aw]: [f32; 4], [bx, by, bz, bw]: [f32; 4], sign: f32| {
        [sign.mul_add(bx, ax), sign.mul_add(by, ay), sign.mul_add(bz, az), sign.mul_add(bw, aw)]
    };
    // Left, right, bottom and top from the clip edges, and the near plane from how far along the view a point
    // has to be to draw at all.
    let near = [xw, yw, zw, ww - NEAR];
    [edge(along, clip_x, 1.0), edge(along, clip_x, -1.0), edge(along, clip_y, 1.0), edge(along, clip_y, -1.0), near]
}

/// Whether the box between two corners shows on a screen with these `sides`.
pub(crate) fn in_view(sides: &[[f32; 4]; 5], [low, high]: [[f32; 3]; 2]) -> bool {
    let ([low_x, low_y, low_z], [high_x, high_y, high_z]) = (low, high);
    sides.iter().all(|&[toward_x, toward_y, toward_z, offset]| {
        // The corner of the box furthest along the plane's normal: if even that one is behind the plane, the
        // whole box is.
        let corner = |toward: f32, low: f32, high: f32| if toward >= 0.0 { high } else { low };
        let (x, y) = (corner(toward_x, low_x, high_x), corner(toward_y, low_y, high_y));
        let z = corner(toward_z, low_z, high_z);
        toward_x.mul_add(x, toward_y.mul_add(y, toward_z * z)) + offset >= 0.0
    })
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
    fn only_what_falls_on_the_screen_is_kept() {
        // Looking along +X, half a right angle wide on a square screen.
        let matrix = view_projection([0, 0, 0], 90.0, 1.0);
        let sides = sides(&matrix);
        let box_at = |x: f32, y: f32| [[x - 10.0, y - 10.0, -10.0], [x + 10.0, y + 10.0, 10.0]];
        assert!(in_view(&sides, box_at(500.0, 0.0)), "straight ahead is seen");
        assert!(!in_view(&sides, box_at(-500.0, 0.0)), "behind the camera is not");
        assert!(!in_view(&sides, box_at(100.0, 900.0)), "far off to the side is not");
        assert!(in_view(&sides, box_at(100.0, 80.0)), "a little off to the side still is");
        // A box that reaches from behind to in front is kept, since part of it shows.
        assert!(in_view(&sides, [[-500.0, -10.0, -10.0], [500.0, 10.0, 10.0]]));
    }

    #[test]
    fn places_vertices_scaled_rotated_and_moved() {
        let axes = axes([0, 16384, 0]);
        assert!(close(place([1.0, 0.0, 2.0], [2.0, 1.0, 3.0], &axes, [10.0, 0.0, 0.0]), [10.0, 2.0, 6.0]));
    }
}

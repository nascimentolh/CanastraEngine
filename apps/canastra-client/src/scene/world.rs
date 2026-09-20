//! The world's map tiles: which one holds a place, and where the camera stands to watch a character there.

use ue2_level::Placement;

use super::camera;

/// World units a tile spans on X and Y, and the tile that starts at zero. Both are measured from the client's
/// own maps, whose terrains sit half a tile in: `17_25`'s is centered at (-81920, 245760), `18_25`'s a tile
/// east at (-49152, 245760) and `20_21`'s at (16384, 114688).
const TILE: f32 = 32768.0;
const ORIGIN: [i32; 2] = [20, 18];

/// How far behind the character the camera stands, as H5's own `Engine.u` places it: `PlayerCalcView` asks
/// `CalcBehindView` for 250 units and that pulls the camera 30 back in, level with what it watches.
const DISTANCE: f32 = 250.0 - 30.0;

/// The map file holding `location`.
pub(crate) fn map_at([x, y, _]: [f32; 3]) -> String {
    #[expect(clippy::cast_possible_truncation, reason = "the world spans a few dozen tiles")]
    let tile = |value: f32, origin: i32| (value / TILE).floor() as i32 + origin;
    format!("{}_{}.unr", tile(x, ORIGIN[0]), tile(y, ORIGIN[1]))
}

/// Where the camera stands to watch a character standing at `at` facing `yaw`, whose body reaches `middle`
/// above its feet: level with that middle, `DISTANCE` behind it, as H5 places a camera nobody has tilted yet.
pub(crate) fn behind(at: [f32; 3], yaw: i32, middle: f32) -> Placement {
    let [forward, _, _] = camera::axes([0, yaw, 0]);
    let location = [at[0] - forward[0] * DISTANCE, at[1] - forward[1] * DISTANCE, at[2] + middle];
    Placement { location, rotation: [0, yaw, 0] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_fall_in_the_tile_whose_terrain_holds_them() {
        // The terrain centers the client's own maps store.
        assert_eq!(map_at([-81_920.0, 245_760.0, 0.0]), "17_25.unr");
        assert_eq!(map_at([-49_152.0, 245_760.0, 0.0]), "18_25.unr");
        assert_eq!(map_at([16_384.0, 114_688.0, 0.0]), "20_21.unr");
        // Talking Island's human starting point, where characters are created.
        assert_eq!(map_at([-71_338.0, 258_271.0, -3_104.0]), "17_25.unr");
        // A tile holds its own near corner and leaves the next tile's to it.
        assert_eq!(map_at([-98_304.0, 229_376.0, 0.0]), "17_25.unr");
        assert_eq!(map_at([-65_536.0, 229_376.0, 0.0]), "18_25.unr");
    }

    #[test]
    fn the_camera_stands_level_behind_the_character_at_its_middle() {
        let at = [100.0, 50.0, 10.0];
        let middle = 23.5;
        let placement = behind(at, 0, middle);
        let expected = [at[0] - DISTANCE, at[1], at[2] + middle];
        assert!(placement.location.iter().zip(expected).all(|(at, expected)| (at - expected).abs() < 0.01));
        assert_eq!(placement.rotation, [0, 0, 0], "the camera looks level, the way the character faces");
        // A character facing a quarter turn is watched from behind on the other axis.
        let placement = behind(at, 16_384, middle);
        assert!((placement.location[1] - (at[1] - DISTANCE)).abs() < 0.01);
        assert_eq!(placement.rotation[1], 16_384);
    }
}

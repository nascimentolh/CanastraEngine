//! The ground the world stands on: geodata tiles that say how high the floor is under a place, which way a
//! character may leave each cell, and what stands between two places.
//!
//! A move steps from cell to cell along a straight line: each step has to be allowed by the side it leaves
//! through, a diagonal step also needs both of the straight steps around it, and no step may climb more than
//! [`STEP`]. A move that cannot be finished ends at the last cell it reached.
//!
//! Sight runs from one character's eyes to the other's, and the ground between them may not rise over that
//! line by more than [`OBSTACLE`]. It is checked both ways, since each of the two sees the other only when
//! the line is clear from where it stands.

#![expect(clippy::cast_precision_loss, reason = "heights and cell counts are small whole numbers")]

mod cells;
mod line;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cells::{CELL, Cell, EAST, NORTH, REGION_CELLS, Region, SOUTH, WEST};
use line::Line;

/// Where the world's cells start, so the lowest place in the world is cell zero.
const WORLD_MIN_X: i32 = -655_360;
const WORLD_MIN_Y: i32 = -589_824;
/// How far a character climbs in one step of a cell, in world units.
const STEP: i32 = 48;
/// Half a cell's height, which a place may stand over its floor and still belong to it.
const HALF_CELL: i32 = 8;
/// How far a character's eyes are up its body, of the body's whole height.
const EYES: f32 = 0.75;
/// How far the ground may rise over a line of sight and still be seen over, in world units.
const OBSTACLE: f32 = 32.0;

/// The world's geodata, with the tiles read so far.
pub(crate) struct Geo {
    folder: PathBuf,
    /// Tiles by their name, read the first time a place in one is asked about; `None` where there is no file.
    // ponytail: a tile is read where it is asked for, which holds that player's task for a few milliseconds;
    // read them on a blocking task if it ever shows.
    tiles: Mutex<HashMap<[i32; 2], Option<Region>>>,
}

impl Geo {
    /// The geodata in `folder`, whose files are named after the map tiles they cover, such as `17_25.l2j`.
    pub(crate) fn open(folder: &Path) -> Self {
        Self { folder: folder.to_path_buf(), tiles: Mutex::new(HashMap::new()) }
    }

    /// The floor a character at `at` stands on: the one under its feet, allowing half a cell above them, so a
    /// character indoors keeps the floor it is on instead of the roof over it. Where the world has no geodata,
    /// or nothing is built under the place, it stands where it is.
    pub(crate) fn spawn_height(&self, at: [i32; 3]) -> i32 {
        let [x, y, z] = at;
        let floors = self.floors([geo_x(x), geo_y(y)]);
        if floors.is_empty() {
            return z;
        }
        below(&floors, z + HALF_CELL).or_else(|| nearest(&floors, z)).map_or(z, |floor| floor.height)
    }

    /// Whether two characters can see each other. Each looks from its eyes, which sit `EYES` of the way up a
    /// body `body` units to its middle. The line is checked from both ends.
    // ponytail: nothing in the world is looked at yet; the first target or npc is what calls this.
    #[allow(dead_code, reason = "the world has no targets yet")]
    pub(crate) fn can_see(&self, from: [i32; 3], from_body: f32, to: [i32; 3], to_body: f32) -> bool {
        self.sees(from, from_body, to, to_body) && self.sees(to, to_body, from, from_body)
    }

    /// Whether a character at `from` sees one at `to`: the ground between their eyes may not rise over the
    /// line between them.
    fn sees(&self, from: [i32; 3], from_body: f32, to: [i32; 3], to_body: f32) -> bool {
        let (start, end) = ([geo_x(from[0]), geo_y(from[1])], [geo_x(to[0]), geo_y(to[1])]);
        let (standing, watched) = (self.floors(start), self.floors(end));
        if standing.is_empty() || watched.is_empty() {
            // Nothing is built there to stand in the way.
            return true;
        }
        let (Some(mut floor), Some(target)) =
            (below(&standing, from[2] + HALF_CELL), below(&watched, to[2] + HALF_CELL))
        else {
            return false;
        };
        if start == end {
            return floor.height == target.height;
        }
        let eyes = |floor: Cell, body: f32| floor.height as f32 + 2.0 * body * EYES;
        let (from_eyes, to_eyes) = (eyes(floor, from_body), eyes(target, to_body));
        let away = |[x, y]: [i32; 2]| {
            let (sideways, along) = ((x - start[0]) as f32, (y - start[1]) as f32);
            sideways.mul_add(sideways, along * along).sqrt()
        };
        let climb = (to_eyes - from_eyes) / away(end).max(1.0);
        let mut at = start;
        for step in Line::new(start, end) {
            // The ground of the next cell, as the line meets it: over a wall, it is the top of the wall.
            let floors = self.floors(step);
            let walled = floor.sides & side(at, step) == 0;
            let next = if walled { above(&floors, floor.height - CELL) } else { below(&floors, floor.height + STEP) };
            let Some(next) = next else { return false };
            if next.height as f32 > climb.mul_add(away(step), from_eyes) + OBSTACLE {
                return false;
            }
            (at, floor) = (step, next);
        }
        true
    }

    /// Where a character walking from `from` toward `to` really ends: `to` when the way is clear, and
    /// otherwise the last place along the way it could reach.
    pub(crate) fn walk(&self, from: [i32; 3], to: [i32; 3]) -> [i32; 3] {
        let (start, end) = ([geo_x(from[0]), geo_y(from[1])], [geo_x(to[0]), geo_y(to[1])]);
        let Some(first) = self.cell(start, from[2]) else { return to };
        let (mut at, mut height, mut sides) = (start, first.height, first.sides);
        let mut reached = [from[0], from[1], height];
        for step in Line::new(start, end) {
            let Some(next) = self.cell(step, height) else {
                // Beyond the world's geodata nothing blocks; the walk ends where it was asked to.
                return to;
            };
            if !self.may_leave(at, height, sides, step) || (next.height - height).abs() > STEP {
                return reached;
            }
            (at, height, sides) = (step, next.height, next.sides);
            reached = [world_x(step[0]), world_y(step[1]), height];
        }
        // The destination keeps the place asked for, at the height the floor there has.
        [to[0], to[1], height]
    }

    /// Whether a character standing on `at` may step to `next`: the side it leaves by has to be open, and a
    /// diagonal step also needs the two straight steps that make it.
    fn may_leave(&self, at: [i32; 2], height: i32, sides: u8, next: [i32; 2]) -> bool {
        let leaving = side(at, next);
        let (sideways, along) = (leaving & (EAST | WEST), leaving & (NORTH | SOUTH));
        if sides & leaving != leaving {
            return false;
        }
        if sideways == 0 || along == 0 {
            return true;
        }
        // Both ways round the corner have to be open, or a character would slip through a wall's edge.
        let beside = self.cell([at[0], next[1]], height).is_some_and(|cell| cell.sides & sideways == sideways);
        let ahead = self.cell([next[0], at[1]], height).is_some_and(|cell| cell.sides & along == along);
        beside && ahead
    }

    /// The floor nearest the height `z` in the cell `at`, with the sides it opens onto; `None` outside the
    /// world's geodata. Cells are counted from the world's corner, not in world units.
    fn cell(&self, at: [i32; 2], z: i32) -> Option<Cell> {
        self.floors(at).into_iter().min_by_key(|floor| (floor.height - z).abs())
    }

    /// The floors of a cell, lowest first; empty outside the world's geodata.
    fn floors(&self, [geo_x, geo_y]: [i32; 2]) -> Vec<Cell> {
        let tile = [geo_x.div_euclid(REGION_CELLS), geo_y.div_euclid(REGION_CELLS)];
        let within = |geo: i32| usize::try_from(geo.rem_euclid(REGION_CELLS)).unwrap_or_default();
        self.in_tile(tile, |region| Some(region.floors([within(geo_x), within(geo_y)]))).unwrap_or_default()
    }

    /// Runs `read` on the tile holding a place, reading its file the first time it is asked for.
    fn in_tile<T>(&self, tile: [i32; 2], read: impl FnOnce(&Region) -> Option<T>) -> Option<T> {
        let mut tiles = self.tiles.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let region = tiles.entry(tile).or_insert_with(|| {
            let path = self.folder.join(format!("{}_{}.l2j", tile[0], tile[1]));
            let read = std::fs::read(&path).map_err(|error| error.to_string()).and_then(Region::read);
            match read {
                Ok(region) => {
                    tracing::info!(tile = format!("{}_{}", tile[0], tile[1]), "geodata tile read");
                    Some(region)
                }
                Err(error) => {
                    tracing::debug!(%error, path = %path.display(), "no geodata for this tile");
                    None
                }
            }
        });
        read(region.as_ref()?)
    }
}

/// The floor of `floors` nearest `z`, the highest at or under it, and the lowest over it.
fn nearest(floors: &[Cell], z: i32) -> Option<Cell> {
    floors.iter().min_by_key(|floor| (floor.height - z).abs()).copied()
}

fn below(floors: &[Cell], z: i32) -> Option<Cell> {
    floors.iter().filter(|floor| floor.height <= z).max_by_key(|floor| floor.height).copied()
}

fn above(floors: &[Cell], z: i32) -> Option<Cell> {
    floors.iter().filter(|floor| floor.height > z).min_by_key(|floor| floor.height).copied()
}

/// The side a step from `at` to `next` leaves by, as a cell marks the sides it opens onto.
fn side(at: [i32; 2], next: [i32; 2]) -> u8 {
    let sideways = match (next[0] - at[0]).signum() {
        1 => EAST,
        -1 => WEST,
        _ => 0,
    };
    let along = match (next[1] - at[1]).signum() {
        1 => SOUTH,
        -1 => NORTH,
        _ => 0,
    };
    sideways | along
}

/// The cell a world place falls in, and the middle of the world a cell covers.
fn geo_x(x: i32) -> i32 {
    (x - WORLD_MIN_X).div_euclid(CELL)
}

fn geo_y(y: i32) -> i32 {
    (y - WORLD_MIN_Y).div_euclid(CELL)
}

fn world_x(geo_x: i32) -> i32 {
    geo_x * CELL + WORLD_MIN_X + CELL / 2
}

fn world_y(geo_y: i32) -> i32 {
    geo_y * CELL + WORLD_MIN_Y + CELL / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_and_cells_line_up_with_the_map_tiles() {
        // Talking Island's human starting point falls in tile 17_25, as the client's own map of it is named.
        let (x, y) = (geo_x(-71_338), geo_y(258_271));
        assert_eq!([x / REGION_CELLS, y / REGION_CELLS], [17, 25]);
        // A cell covers sixteen units, and its world place is the middle of it.
        assert!((world_x(geo_x(-71_338)) - -71_338).abs() <= CELL / 2);
        assert_eq!(geo_x(world_x(x)), x, "a cell's middle falls back in the same cell");
        assert_eq!(geo_x(0), 40_960, "the world's zero sits where tile 20 begins");
    }

    /// Needs the world's geodata: `CANASTRA_TEST_GEODATA=<folder> cargo test -p canastra-game -- --ignored`.
    #[test]
    #[ignore = "needs a folder of geodata tiles in CANASTRA_TEST_GEODATA"]
    fn the_world_reads_as_its_own_maps_have_it() {
        let folder = std::env::var("CANASTRA_TEST_GEODATA").expect("CANASTRA_TEST_GEODATA is set");
        let geo = Geo::open(Path::new(&folder));
        // Talking Island's temple, where human characters are created, stands at about -3104.
        let spawn = [-71_338, 258_271, -3104];
        let floor = geo.spawn_height(spawn);
        assert!((floor - spawn[2]).abs() <= 24, "the temple floor is at {floor}");
        // A character sees along the temple floor it stands on, and not through its walls.
        let body = 23.5;
        let ahead = [spawn[0] + 200, spawn[1], spawn[2]];
        assert!(geo.can_see(spawn, body, ahead, body), "the floor ahead is in plain sight");
        let outside = [spawn[0], spawn[1] - 1500, spawn[2]];
        assert!(!geo.can_see(spawn, body, outside, body), "the wall between them is not seen through");
        // The temple's walls stop a walk well before a place a thousand units outside it.
        let outside = [spawn[0], spawn[1] - 1000, spawn[2]];
        let stopped = geo.walk(spawn, outside);
        assert_ne!(stopped, outside, "a wall stands in the way");
        assert!(stopped[1] > outside[1], "and the walk ends on this side of it");
        // The way it was created facing, into the open temple floor, is clear for a few cells at least.
        let ahead = [spawn[0] + 100, spawn[1], spawn[2]];
        assert_eq!(geo.walk(spawn, ahead), ahead, "the floor ahead of it is walkable");
    }

    #[test]
    fn a_walk_beyond_the_geodata_ends_where_it_was_asked_to() {
        // An empty folder has no tiles at all, so nothing stands in the way.
        let geo = Geo::open(Path::new("geodata-that-is-not-there"));
        let to = [100, 200, 300];
        assert_eq!(geo.walk([0, 0, 0], to), to);
        assert_eq!(geo.spawn_height([0, 0, -3104]), -3104, "and a character stands where it was left");
        assert!(geo.can_see([0, 0, 0], 23.5, to, 23.5), "with nothing built, nothing blocks sight");
    }
}

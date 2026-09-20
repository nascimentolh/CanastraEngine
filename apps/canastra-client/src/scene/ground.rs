//! The ground of a map: the level's own floors, kept to stand characters on and to pick a place to walk to.

use std::collections::HashMap;

/// Cells the map is cut into on X and Y, so finding the floors over a place walks a few triangles, not every one.
const CELL: f32 = 256.0;
/// How far above a place a floor may still be the one it stands on: a step, so a character on a platform is not
/// pulled up to the roof above it.
const STEP: f32 = 64.0;
/// How flat a triangle must be to count as a floor, as the cosine of its slope: half, which is a slope of 60
/// degrees, so ramps count and walls do not.
const FLAT: f32 = 0.5;
/// How far from where the map was loaded its floors are kept, in world units. A map tile is twice this
/// across, and a character walks into the next tile, which is read afresh, before it runs out of floors.
const KEPT: f32 = 16_384.0;

/// The floors of a map, in the same places as the scene's vertices, grouped by the cell they fall in.
// ponytail: floors only, and a ray meets them by sampling along it; walls and ceilings are left out, so a click
// on a wall lands on the floor behind it.
#[derive(Default)]
pub(crate) struct Ground {
    /// Three corners a triangle, all of them floors.
    corners: Vec<[[f32; 3]; 3]>,
    /// Which triangles fall in each cell.
    cells: HashMap<[i32; 2], Vec<u32>>,
}

impl Ground {
    /// The floors among the triangles `indices` makes of `positions`, a scene's level geometry.
    pub(crate) fn new(positions: &[[f32; 3]], indices: &[u32]) -> Self {
        let mut ground = Self::default();
        for triangle in indices.as_chunks::<3>().0 {
            let corners = triangle.map(|index| positions.get(index as usize).copied().unwrap_or_default());
            if !is_floor(corners) || !within(corners, KEPT) {
                continue;
            }
            let index = u32::try_from(ground.corners.len()).unwrap_or(u32::MAX);
            for cell in cells_of(corners) {
                ground.cells.entry(cell).or_default().push(index);
            }
            ground.corners.push(corners);
        }
        ground
    }

    /// The floor a character at `at` stands on: the highest one under its feet, or within a step above them.
    pub(crate) fn under(&self, [x, y, z]: [f32; 3]) -> Option<f32> {
        self.cells
            .get(&cell_of([x, y]))?
            .iter()
            .filter_map(|index| height_at(self.corners.get(*index as usize)?, [x, y]))
            .filter(|height| *height <= z + STEP)
            .max_by(f32::total_cmp)
    }

    /// Where a ray from `eye` along `direction` first goes under a floor, up to `reach` away.
    pub(crate) fn hit(&self, eye: [f32; 3], direction: [f32; 3], reach: f32) -> Option<[f32; 3]> {
        let step = CELL / 2.0;
        let along = |distance: f32| {
            let mut at = eye;
            for (at, direction) in at.iter_mut().zip(direction) {
                *at += direction * distance;
            }
            at
        };
        // The ray is under a floor where the nearest one at or just above it stands over it.
        let below = |distance: f32| {
            let [x, y, z] = along(distance);
            self.under([x, y, z]).filter(|height| z <= *height)
        };
        let mut walked = step;
        while walked < reach {
            if below(walked).is_some() {
                // The ray crossed a floor since the last sample: close in on where it crossed.
                let (mut over, mut under) = (walked - step, walked);
                for _ in 0..12 {
                    let middle = over.midpoint(under);
                    if below(middle).is_some() { under = middle } else { over = middle }
                }
                let [x, y, z] = along(under);
                return Some([x, y, below(under).unwrap_or(z)]);
            }
            walked += step;
        }
        None
    }
}

/// Whether a triangle stands within `reach` of where the map was loaded, which is where the scene's own
/// places are measured from.
fn within([first, _, _]: [[f32; 3]; 3], reach: f32) -> bool {
    let (x, y) = (first.first().copied().unwrap_or_default(), first.get(1).copied().unwrap_or_default());
    x.mul_add(x, y * y) <= reach * reach
}

/// Whether a triangle lies flat enough to stand on and faces up, so ceilings are not floors. The client winds
/// its triangles so that this cross product points into the surface: a floor's points down.
fn is_floor([first, second, third]: [[f32; 3]; 3]) -> bool {
    let (along, across) = (sub(second, first), sub(third, first));
    let normal = [
        along[1] * across[2] - along[2] * across[1],
        along[2] * across[0] - along[0] * across[2],
        along[0] * across[1] - along[1] * across[0],
    ];
    let length = normal.iter().map(|value| value * value).sum::<f32>().sqrt();
    length > 0.0 && -normal[2] / length > FLAT
}

/// The height of a triangle over `[x, y]`, when that point falls inside it.
fn height_at([first, second, third]: &[[f32; 3]; 3], [x, y]: [f32; 2]) -> Option<f32> {
    let area = (second[0] - first[0]) * (third[1] - first[1]) - (third[0] - first[0]) * (second[1] - first[1]);
    if area.abs() < f32::EPSILON {
        return None;
    }
    let to_second = ((x - first[0]) * (third[1] - first[1]) - (third[0] - first[0]) * (y - first[1])) / area;
    let to_third = ((second[0] - first[0]) * (y - first[1]) - (x - first[0]) * (second[1] - first[1])) / area;
    let to_first = 1.0 - to_second - to_third;
    let inside = [to_first, to_second, to_third].iter().all(|weight| *weight >= -1e-4);
    inside.then(|| first[2] * to_first + second[2] * to_second + third[2] * to_third)
}

fn cell_of([x, y]: [f32; 2]) -> [i32; 2] {
    #[expect(clippy::cast_possible_truncation, reason = "a map is a few hundred cells across")]
    [(x / CELL).floor() as i32, (y / CELL).floor() as i32]
}

/// Every cell a triangle reaches into.
fn cells_of([first, second, third]: [[f32; 3]; 3]) -> Vec<[i32; 2]> {
    let ([ax, ay, _], [bx, by, _], [cx, cy, _]) = (first, second, third);
    let low = cell_of([ax.min(bx).min(cx), ay.min(by).min(cy)]);
    let high = cell_of([ax.max(bx).max(cx), ay.max(by).max(cy)]);
    (low[0]..=high[0]).flat_map(|x| (low[1]..=high[1]).map(move |y| [x, y])).collect()
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two floors: one at zero over a wide square, and one at 100 over its left half.
    fn ground() -> Ground {
        let positions = [
            [-500.0, -500.0, 0.0],
            [500.0, -500.0, 0.0],
            [-500.0, 500.0, 0.0],
            [500.0, 500.0, 0.0],
            [-500.0, -500.0, 100.0],
            [0.0, -500.0, 100.0],
            [-500.0, 500.0, 100.0],
            [0.0, 500.0, 100.0],
        ];
        // Wound as the client winds its floors, so their normals point down.
        let indices = [0, 2, 1, 1, 2, 3, 4, 6, 5, 5, 6, 7];
        Ground::new(&positions, &indices)
    }

    #[test]
    fn a_place_stands_on_the_floor_under_it_and_not_on_the_one_above() {
        let ground = ground();
        assert_eq!(ground.under([250.0, 0.0, 0.0]), Some(0.0), "the open half has one floor");
        assert_eq!(ground.under([-250.0, 0.0, 100.0]), Some(100.0), "standing on the upper floor keeps it");
        assert_eq!(ground.under([-250.0, 0.0, 0.0]), Some(0.0), "standing below it keeps the lower one");
        assert_eq!(ground.under([5000.0, 0.0, 0.0]), None, "nothing is built out there");
    }

    #[test]
    fn a_ray_lands_where_it_meets_the_floor() {
        let ground = ground();
        let hit = ground.hit([250.0, 0.0, 500.0], [0.0, 0.0, -1.0], 2000.0).expect("straight down lands");
        assert!(hit[2].abs() < 1.0 && (hit[0] - 250.0).abs() < 1.0, "landed at {hit:?}");
        // Slanting down from over the upper floor lands on it, not on the one below.
        let slope = 1.0 / 2.0_f32.sqrt();
        let hit = ground.hit([-400.0, 0.0, 400.0], [slope, 0.0, -slope], 2000.0).expect("a slanted ray lands");
        assert!((hit[2] - 100.0).abs() < 2.0, "landed at {hit:?}");
        assert!(ground.hit([250.0, 0.0, 500.0], [0.0, 0.0, 1.0], 2000.0).is_none(), "looking up meets nothing");
    }
}

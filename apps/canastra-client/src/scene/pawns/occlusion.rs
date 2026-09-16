//! How much sky each of a character's vertices sees, baked once from the character's own parts.
//!
//! H5 keeps the cloth under a tabard and the inside of a skirt far darker than the plates facing out. Nothing
//! in the client's data says so: it falls out of the shape. Rays from each vertex are counted against the
//! character's own triangles, held in a grid of the boxes they fill, and the share that escapes dims the sky
//! light on that vertex.
// ponytail: baked in the bind pose and only against the character itself; bake again if characters ever move
// enough for it to show, and add the scene's own shadows separately.

/// Rays cast from each vertex, spread over the hemisphere around its normal.
const RAYS: usize = 16;
/// Boxes along the character's longest side.
const GRID: usize = 96;
/// How far a ray looks, as a share of the character's height: enough to find the piece above it, not the ground.
const REACH: f32 = 0.16;
/// How dark the most enclosed vertex goes, from 0 (black) to 1 (untouched).
const DARKEST: f32 = 0.35;

use ue2_assets::SkinVertex;

/// A part of the character in the space of its mesh: its vertices and the triangles over them.
pub(super) struct Shape<'a> {
    pub(super) vertices: &'a [SkinVertex],
    pub(super) triangles: &'a [u16],
}

/// How much sky each vertex of each shape sees, from `DARKEST` to 1.
pub(super) fn sky(shapes: &[Shape<'_>]) -> Vec<Vec<f32>> {
    let Some(grid) = Grid::of(shapes) else {
        return shapes.iter().map(|shape| vec![1.0; shape.vertices.len()]).collect();
    };
    let reach = grid.side * GRID as f32 * REACH;
    shapes
        .iter()
        .map(|shape| {
            shape
                .vertices
                .iter()
                .map(|vertex| {
                    // The ray starts a box out along the normal, or it finds the vertex's own surface.
                    let normal = normalized(vertex.normal);
                    let mut from = vertex.position;
                    for ((from, position), normal) in from.iter_mut().zip(vertex.position).zip(normal) {
                        *from = position + normal * grid.side * 1.0;
                    }
                    let open = directions(vertex.normal).filter(|&way| !grid.blocked(from, way, reach)).count();
                    DARKEST + (1.0 - DARKEST) * open as f32 / RAYS as f32
                })
                .collect()
        })
        .collect()
}

/// `RAYS` directions spread over the hemisphere around `normal`, by the golden angle.
fn directions(normal: [f32; 3]) -> impl Iterator<Item = [f32; 3]> {
    let [nx, ny, nz] = normalized(normal);
    // A frame around the normal, built from whichever axis it leans on least.
    let aside = if nz.abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let right = normalized(cross([nx, ny, nz], aside));
    let up = cross([nx, ny, nz], right);
    (0..RAYS).map(move |ray| {
        let cosine = (ray as f32 + 0.5) / RAYS as f32;
        let sine = (1.0 - cosine * cosine).sqrt();
        let turn = ray as f32 * 2.399_963_2;
        let (across, along) = (turn.cos() * sine, turn.sin() * sine);
        [
            nx * cosine + right[0] * across + up[0] * along,
            ny * cosine + right[1] * across + up[1] * along,
            nz * cosine + right[2] * across + up[2] * along,
        ]
    })
}

/// The boxes the character's triangles fill, to walk rays through.
struct Grid {
    filled: Vec<bool>,
    lowest: [f32; 3],
    side: f32,
    size: [usize; 3],
}

impl Grid {
    fn of(shapes: &[Shape<'_>]) -> Option<Self> {
        let points = || shapes.iter().flat_map(|shape| shape.vertices).map(|vertex| vertex.position);
        let mut lowest = [f32::MAX; 3];
        let mut highest = [f32::MIN; 3];
        for point in points() {
            for ((low, high), value) in lowest.iter_mut().zip(&mut highest).zip(point) {
                (*low, *high) = (low.min(value), high.max(value));
            }
        }
        let mut span = highest;
        for ((span, high), low) in span.iter_mut().zip(highest).zip(lowest) {
            *span = high - low;
        }
        let longest = span.iter().copied().fold(0.0_f32, f32::max);
        if longest <= 0.0 {
            return None;
        }
        let side = longest / GRID as f32;
        let boxes = span.map(|length| steps(length / side) + 1);
        let mut grid = Self { filled: vec![false; boxes[0] * boxes[1] * boxes[2]], lowest, side, size: boxes };
        for shape in shapes {
            for triangle in shape.triangles.as_chunks::<3>().0 {
                let corners: Vec<[f32; 3]> = triangle
                    .iter()
                    .filter_map(|&index| shape.vertices.get(usize::from(index)).map(|vertex| vertex.position))
                    .collect();
                let [first, second, third] = corners.as_slice() else { continue };
                grid.fill([*first, *second, *third]);
            }
        }
        Some(grid)
    }

    /// Marks the boxes a triangle passes through, walking it in steps smaller than a box.
    fn fill(&mut self, [corner, along, across]: [[f32; 3]; 3]) {
        let sides = [distance(corner, along), distance(corner, across), distance(along, across)];
        let longest = sides.into_iter().fold(0.0_f32, f32::max);
        let steps = (steps(longest / self.side) + 1).min(16);
        for down in 0..=steps {
            for over in 0..=(steps - down) {
                let (down, over) = (down as f32 / steps as f32, over as f32 / steps as f32);
                let mut point = corner;
                for (((point, corner), along), across) in point.iter_mut().zip(corner).zip(along).zip(across) {
                    *point = corner + (along - corner) * down + (across - corner) * over;
                }
                if let Some(filled) = self.index(point).and_then(|index| self.filled.get_mut(index)) {
                    *filled = true;
                }
            }
        }
    }

    #[expect(clippy::cast_possible_truncation, reason = "a box along one axis, checked against the count")]
    fn index(&self, point: [f32; 3]) -> Option<usize> {
        let mut index = 0;
        let axes = point.iter().zip(self.lowest).zip(self.size).rev();
        for ((point, lowest), boxes) in axes {
            let box_of = usize::try_from(((point - lowest) / self.side).floor() as i64).ok()?;
            if box_of >= boxes {
                return None;
            }
            index = index * boxes + box_of;
        }
        Some(index)
    }

    /// Whether anything stands within `reach` along `way` from `from`, leaving the vertex's own box out.
    fn blocked(&self, from: [f32; 3], way: [f32; 3], reach: f32) -> bool {
        let way = normalized(way);
        let steps = steps(reach / (self.side * 0.7));
        (1..=steps).any(|step| {
            let travelled = self.side * 0.7 * step as f32;
            let mut point = from;
            for ((point, from), way) in point.iter_mut().zip(from).zip(way) {
                *point = from + way * travelled;
            }
            self.index(point).and_then(|index| self.filled.get(index)).copied().unwrap_or(false)
        })
    }
}

/// How many whole steps of `count` there are, at least one.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a count of boxes, never huge or negative")]
fn steps(count: f32) -> usize {
    (count.ceil().max(1.0)) as usize
}

fn normalized(vector: [f32; 3]) -> [f32; 3] {
    let length = vector.iter().map(|value| value * value).sum::<f32>().sqrt().max(f32::EPSILON);
    vector.map(|value| value / length)
}

fn cross([ax, ay, az]: [f32; 3], [bx, by, bz]: [f32; 3]) -> [f32; 3] {
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(a, b)| (a - b) * (a - b)).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vertex_under_a_lid_sees_less_sky_than_one_in_the_open() {
        // A wide lid over the origin, and two points facing up: one under it, one well to the side.
        let vertex = |position, normal| SkinVertex {
            position,
            normal,
            uv: [0.0; 2],
            bones: [0; 4],
            weights: [1.0, 0.0, 0.0, 0.0],
        };
        let lid_vertices = [
            vertex([-20.0, -20.0, 4.0], [0.0, 0.0, -1.0]),
            vertex([20.0, -20.0, 4.0], [0.0, 0.0, -1.0]),
            vertex([0.0, 20.0, 4.0], [0.0, 0.0, -1.0]),
        ];
        let below = [vertex([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]), vertex([40.0, 0.0, 0.0], [0.0, 0.0, 1.0])];
        let lid = Shape { vertices: &lid_vertices, triangles: &[0, 1, 2] };
        let points = Shape { vertices: &below, triangles: &[] };
        let baked = sky(&[lid, points]);
        let (under, open) = (baked[1][0], baked[1][1]);
        assert!(under < open, "under the lid {under} is darker than in the open {open}");
        assert!(open > 0.9 && under >= DARKEST);
    }
}

//! The camera moving between the views of a map, along the shots of its scenes.

use std::time::Instant;

use ue2_level::{Placement, Shot};

use super::Scene;
use super::camera::{self, Matrix};

/// How the camera gets to a view, by a scene's tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Route {
    /// Along the scene's shots.
    Play(String),
    /// Along the scene's shots backwards.
    Back(String),
    /// Straight to where the scene ends.
    Cut(String),
}

pub(super) struct Flight {
    legs: Vec<Leg>,
    started: Instant,
}

/// The camera going from one shot to the next.
#[derive(Debug, Clone, Copy)]
struct Leg {
    from: Placement,
    to: Placement,
    /// The curve's two inner points, in the world, when the path curves.
    curve: Option<[[f32; 3]; 2]>,
    seconds: f32,
}

impl Scene {
    /// Takes the camera along the first of `routes` whose scene the map has.
    pub(crate) fn travel(&mut self, routes: &[Route]) {
        let found = routes.iter().find_map(|route| {
            let tag = match route {
                Route::Play(tag) | Route::Back(tag) | Route::Cut(tag) => tag,
            };
            Some((route, self.shots.get(&tag.to_ascii_lowercase())?))
        });
        let Some((route, shots)) = found else { return };
        let legs = match route {
            Route::Play(_) => legs(shots),
            Route::Back(_) => legs(shots).into_iter().rev().map(Leg::reversed).collect(),
            Route::Cut(_) => shots.last().map(|shot| Leg::jump(shot.placement)).into_iter().collect(),
        };
        self.flight = Some(Flight { legs, started: Instant::now() });
    }

    /// Moves the camera to where its flight has it now.
    pub(super) fn fly(&mut self) {
        let Some(flight) = &self.flight else { return };
        let mut elapsed = flight.started.elapsed().as_secs_f32();
        for leg in &flight.legs {
            if elapsed < leg.seconds {
                self.eye = leg.at(elapsed / leg.seconds);
                return;
            }
            elapsed -= leg.seconds;
            self.eye = leg.to;
        }
        self.flight = None;
    }

    /// The view and projection of the camera where it stands now, for vertices placed relative to where the map was
    /// loaded from.
    pub(super) fn view(&self, aspect: f32) -> Matrix {
        let [x, y, z, w] = camera::view_projection(self.eye.rotation, super::FOV, aspect);
        let [dx, dy, dz] = mix(&[(1.0, self.eye.location), (-1.0, self.camera)]);
        let mut moved = w;
        for (((value, x), y), z) in moved.iter_mut().zip(x).zip(y).zip(z) {
            *value -= x * dx + y * dy + z * dz;
        }
        [x, y, z, moved]
    }
}

/// The legs from each of `shots` to the next, starting where the first stands.
fn legs(shots: &[Shot]) -> Vec<Leg> {
    let first = shots.first().map(|shot| Leg::jump(shot.placement));
    let moves = shots.windows(2).filter_map(|pair| {
        let [from, to] = pair else { return None };
        let [leaving, _] = from.handles;
        let [_, arriving] = to.handles;
        let (start, end) = (from.placement.location, to.placement.location);
        let curve = to.curved.then(|| [mix(&[(1.0, start), (1.0, leaving)]), mix(&[(1.0, end), (1.0, arriving)])]);
        Some(Leg { from: from.placement, to: to.placement, curve, seconds: to.seconds })
    });
    first.into_iter().chain(moves).collect()
}

impl Leg {
    fn jump(to: Placement) -> Self {
        Self { from: to, to, curve: None, seconds: 0.0 }
    }

    fn reversed(self) -> Self {
        Self { from: self.to, to: self.from, curve: self.curve.map(|[a, b]| [b, a]), ..self }
    }

    /// Where the camera stands `t` of the way along, from 0 to 1.
    // ponytail: the camera keeps an even pace in time along the curve; H5's pace along PathLength is unmeasured.
    fn at(&self, t: f32) -> Placement {
        let [start, end] = [self.from.location, self.to.location];
        let u = 1.0 - t;
        let location = match self.curve {
            Some([a, b]) => mix(&[(u * u * u, start), (3.0 * u * u * t, a), (3.0 * u * t * t, b), (t * t * t, end)]),
            None => mix(&[(u, start), (t, end)]),
        };
        let mut rotation = self.from.rotation;
        for (from, to) in rotation.iter_mut().zip(self.to.rotation) {
            // The short way round, a turn being 65536.
            let delta = (to - *from + 32768).rem_euclid(65536) - 32768;
            *from += (delta as f32 * t) as i32;
        }
        Placement { location, rotation }
    }
}

/// The sum of each point times its weight.
fn mix(points: &[(f32, [f32; 3])]) -> [f32; 3] {
    let mut sum = [0.0; 3];
    for (weight, point) in points {
        for (sum, value) in sum.iter_mut().zip(point) {
            *sum += weight * value;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leg_curves_through_its_handles_and_turns_the_short_way() {
        let shot = |location, rotation, seconds, curved, handles| Shot {
            placement: Placement { location, rotation },
            seconds,
            curved,
            handles,
        };
        let shots = [
            shot([0.0; 3], [0, 65000, 0], 0.0, false, [[0.0, 10.0, 0.0], [0.0; 3]]),
            shot([10.0, 0.0, 0.0], [0, 1000, 0], 2.0, true, [[0.0; 3], [0.0, 10.0, 0.0]]),
        ];
        let legs = legs(&shots);
        assert_eq!(legs.len(), 2);
        let middle = legs[1].at(0.5);
        assert!((middle.location[0] - 5.0).abs() < 1e-4 && (middle.location[1] - 7.5).abs() < 1e-4);
        assert_eq!(middle.rotation[1], 65768, "halfway from 65000 to 1000 the short way");
        assert!(legs[1].reversed().at(1.0).location.iter().all(|axis| axis.abs() < 1e-4));
    }
}

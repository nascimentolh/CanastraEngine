//! How an `L2MovableStaticMeshActor` sways about where it rests. The class is native, but every such actor saved
//! in the lobby maps satisfies `Location = OrgLocation + TranslationMax × sin(2π × TranslationCurrent)` on each
//! axis, and the same for rotation: `Current` is how far through its cycle each axis is, from 0 to 1.

use ue2_assets::{Property, find};

use crate::Placement;

/// One actor's sway.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Movement {
    /// Where the actor rests; its saved placement is one moment of the sway.
    pub origin: Placement,
    pub translation: Sway,
    /// In Unreal rotation units, pitch, yaw and roll.
    pub rotation: Sway,
}

/// A swing on each of three axes: how far either side of rest, how fast, and how far through its cycle at load.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sway {
    pub max: [f32; 3],
    pub rate: [f32; 3],
    pub phase: [f32; 3],
}

impl Sway {
    /// How far from rest each axis stands `time` seconds after load.
    // ponytail: the phase is taken to advance `rate / max` cycles a second, which no saved value can confirm; the
    // lobby's floating ruins then bob every 7 to 10 seconds. Check against a timed H5 capture.
    #[must_use]
    pub fn offset(&self, time: f32) -> [f32; 3] {
        let mut offset = [0.0; 3];
        for (((offset, max), rate), phase) in offset.iter_mut().zip(self.max).zip(self.rate).zip(self.phase) {
            if max != 0.0 {
                let cycles = phase + time * rate.abs() / max.abs();
                *offset = max * (cycles * std::f32::consts::TAU).sin();
            }
        }
        offset
    }
}

/// The sway of an actor with these properties, when it is a movable static mesh actor.
pub(crate) fn read(class: &str, properties: &[Property<'_>], placement: Placement) -> Option<Movement> {
    if !class.eq_ignore_ascii_case("L2MovableStaticMeshActor") {
        return None;
    }
    let get = |name| find(properties, name);
    let vector = |name| get(name).and_then(Property::vector).unwrap_or_default();
    #[expect(clippy::cast_precision_loss, reason = "rotations stay within a few turns")]
    let rotator = |name| get(name).and_then(Property::rotator).unwrap_or_default().map(|value| value as f32);
    let ratio = |name| get(name).and_then(Property::vector).unwrap_or([1.0; 3]);
    let scaled = |[x, y, z]: [f32; 3], [a, b, c]: [f32; 3]| [x * a, y * b, z * c];
    Some(Movement {
        origin: Placement {
            location: get("OrgLocation").and_then(Property::vector).unwrap_or(placement.location),
            rotation: get("OrgRotation").and_then(Property::rotator).unwrap_or(placement.rotation),
        },
        translation: Sway {
            max: scaled(vector("TranslationMax"), ratio("TranslationMaxRatio")),
            rate: vector("TranslationRate"),
            phase: vector("TranslationCurrent"),
        },
        rotation: Sway {
            max: scaled(rotator("RotationMax"), ratio("RotationMaxRatio")),
            rate: rotator("RotationRate"),
            phase: vector("RotationCurrent"),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sway_starts_where_the_level_saved_it_and_swings_either_side() {
        // lobby01's L2MovableStaticMeshActor1: saved 26244 units short of rest at phase 0.852.
        let sway = Sway { max: [32768.0; 3], rate: [4096.0; 3], phase: [0.852_172_4; 3] };
        assert!((sway.offset(0.0)[0] + 26244.0).abs() < 30.0);
        // A quarter cycle from phase zero reaches the full swing: 8 seconds is a whole cycle at 4096 of 32768.
        let from_rest = Sway { phase: [0.0; 3], ..sway };
        assert!((from_rest.offset(2.0)[1] - 32768.0).abs() < 1.0);
        assert!(Sway::default().offset(5.0).iter().all(|axis| axis.abs() < f32::EPSILON), "no swing stays at rest");
    }
}

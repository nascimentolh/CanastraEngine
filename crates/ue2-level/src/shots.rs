//! The camera moves of a scene: each action's interpolation point, in order.

use ue2_assets::{Error, Property, find, object_properties};
use ue2_package::{ObjectRef, Package};

use crate::{Placement, placement};

/// Where one of a scene's actions takes the camera, and how.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    pub placement: Placement,
    /// How long the camera takes to get here from the shot before; a warp takes none.
    pub seconds: f32,
    /// Whether the path here curves, rather than running straight from the shot before.
    pub curved: bool,
    /// The curve's handles around this point, relative to it: the one leaving it, then the one arriving at it.
    pub handles: [[f32; 3]; 2],
}

/// The shots of a scene's warps and camera moves, in order; actions of other kinds are left out.
pub(crate) fn read(package: &Package, file: &[u8], scene: &[Property<'_>]) -> Result<Vec<Shot>, Error> {
    let export = |object: Option<ObjectRef>| match object {
        Some(ObjectRef::Export(index)) => package.exports().get(index),
        _ => None,
    };
    let actions = find(scene, "Actions").and_then(|actions| actions.objects(package)).unwrap_or_default();
    let mut shots = Vec::new();
    for action in actions.into_iter().filter_map(|action| export(Some(action))) {
        let class = package.class_name(action);
        let moves = class.eq_ignore_ascii_case("ActionMoveCamera");
        if !moves && !class.eq_ignore_ascii_case("ActionWarp") {
            continue;
        }
        let action = object_properties(package, file, action)?;
        let Some(point) = export(find(&action, "IntPoint").and_then(|point| point.object(package))) else { continue };
        let point = object_properties(package, file, point)?;
        let vector = |name| find(&action, name).and_then(Property::vector).unwrap_or_default();
        shots.push(Shot {
            placement: placement(&point),
            seconds: if moves { find(&action, "Duration").and_then(Property::float).unwrap_or(0.0) } else { 0.0 },
            curved: moves && find(&action, "PathStyle").and_then(Property::byte) == Some(1),
            handles: [vector("StartControlPoint"), vector("EndControlPoint")],
        });
    }
    Ok(shots)
}

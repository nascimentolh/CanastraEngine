//! Placed actors and scene cameras read from an Unreal Engine 2 map package.
//!
//! Only what the client draws from is extracted: where each actor sits and what it shows. Property
//! values stay in the package; nothing is converted.

use std::collections::BTreeMap;

use ue2_assets::{Error, Property, find, object_properties};
use ue2_package::{ObjectRef, Package};

/// Pitch, yaw and roll in Unreal units, 65536 to a full turn.
pub type Rotator = [i32; 3];

/// A position and facing in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub location: [f32; 3],
    pub rotation: Rotator,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Actor {
    pub class: String,
    pub name: String,
    pub tag: Option<String>,
    /// Editor groups, without the implicit `None`.
    pub groups: Vec<String>,
    pub placement: Placement,
    /// `DrawScale` times `DrawScale3D` on each axis.
    pub scale: [f32; 3],
    /// Object path of the static mesh, e.g. `L2_Lobby.BTree_A`.
    pub static_mesh: Option<String>,
    /// Material overrides by slot, as object paths.
    pub skins: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Level {
    pub actors: Vec<Actor>,
    /// Where each scene that starts with a warp puts the camera, by the scene's tag.
    pub warps: BTreeMap<String, Placement>,
}

/// Reads every placed actor, recognized by the `Level` reference the editor stores in each of them.
pub fn read_level(package: &Package, file: &[u8]) -> Result<Level, Error> {
    let mut level = Level::default();
    let mut scenes = Vec::new();
    for (index, export) in package.exports().iter().enumerate() {
        if export.serial_size == 0 || matches!(export.class, ObjectRef::Null) {
            continue;
        }
        let properties = object_properties(package, file, export)?;
        if find(&properties, "Level").is_none() {
            continue;
        }
        let actor = actor(package, index, &properties);
        if actor.class.eq_ignore_ascii_case("SceneManager") {
            scenes.push((actor.tag.clone(), properties));
        }
        level.actors.push(actor);
    }
    for (tag, properties) in scenes {
        if let (Some(tag), Some(warp)) = (tag, first_warp(package, file, &properties)?) {
            level.warps.insert(tag, warp);
        }
    }
    Ok(level)
}

fn actor(package: &Package, export: usize, properties: &[Property<'_>]) -> Actor {
    let get = |name: &str| find(properties, name);
    let path = |object: ObjectRef| (!matches!(object, ObjectRef::Null)).then(|| package.object_path(object));
    let draw_scale = get("DrawScale").and_then(Property::float).unwrap_or(1.0);
    let scale3d = get("DrawScale3D").and_then(Property::vector).unwrap_or([1.0; 3]);
    let exports = package.exports();
    Actor {
        class: exports.get(export).map(|export| package.class_name(export).to_owned()).unwrap_or_default(),
        name: package.object_name(ObjectRef::Export(export)).to_owned(),
        tag: get("Tag").and_then(|tag| tag.name_value(package)).map(str::to_owned),
        groups: get("Group")
            .and_then(|group| group.name_value(package))
            .map(|groups| groups.split(',').filter(|group| *group != "None").map(str::to_owned).collect())
            .unwrap_or_default(),
        placement: placement(properties),
        scale: scale3d.map(|axis| axis * draw_scale),
        static_mesh: get("StaticMesh").and_then(|mesh| mesh.object(package)).and_then(path),
        skins: get("Skins")
            .and_then(|skins| skins.objects(package))
            .map(|skins| skins.into_iter().filter_map(path).collect())
            .unwrap_or_default(),
    }
}

fn placement(properties: &[Property<'_>]) -> Placement {
    Placement {
        location: find(properties, "Location").and_then(Property::vector).unwrap_or_default(),
        rotation: find(properties, "Rotation").and_then(Property::rotator).unwrap_or_default(),
    }
}

/// The placement of the interpolation point the scene's first action warps to, if it starts with a warp.
fn first_warp(package: &Package, file: &[u8], scene: &[Property<'_>]) -> Result<Option<Placement>, Error> {
    let export = |object: Option<ObjectRef>| match object {
        Some(ObjectRef::Export(index)) => package.exports().get(index),
        _ => None,
    };
    let actions = find(scene, "Actions").and_then(|actions| actions.objects(package)).unwrap_or_default();
    let Some(action) = export(actions.first().copied()) else { return Ok(None) };
    if !package.class_name(action).eq_ignore_ascii_case("ActionWarp") {
        return Ok(None);
    }
    let action = object_properties(package, file, action)?;
    let Some(point) = export(find(&action, "IntPoint").and_then(|point| point.object(package))) else {
        return Ok(None);
    };
    Ok(Some(placement(&object_properties(package, file, point)?)))
}

#[cfg(test)]
mod tests;

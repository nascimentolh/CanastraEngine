//! Placed actors and scene cameras read from an Unreal Engine 2 map package.
//!
//! Only what the client draws from is extracted: where each actor sits and what it shows. Property
//! values stay in the package; nothing is converted.

mod bsp;
mod emitters;
mod lighting;
mod movement;
mod shots;
mod terrain;

use std::collections::BTreeMap;

use ue2_assets::{Error, Property, find, object_properties};
use ue2_package::{ObjectRef, Package};

pub use bsp::BspPolygon;
pub use emitters::{DrawStyle, Emitter, MeshShape, Range, SpriteEmitter};
pub use lighting::TerrainSector;
pub use movement::{Movement, Sway};
pub use shots::Shot;
pub use terrain::{DecoLayer, Terrain, TerrainLayer};

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
    /// Drawn at full brightness, ignoring lighting.
    pub unlit: bool,
    /// Precomputed RGBA lighting per mesh vertex; empty when the level stores none.
    pub lighting: Vec<[u8; 4]>,
    /// How the actor sways, for a movable static mesh actor.
    pub movement: Option<Movement>,
}

/// A sound an `AmbientSoundObject` loops where it stands.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientSound {
    pub location: [f32; 3],
    /// Object path of the sound, e.g. `AmbSound.Dungeon.d_wind_loop_01`.
    pub sound: String,
    /// How far it carries, in world units.
    pub radius: f32,
    /// How loudly it plays where it stands, from 0 to 1.
    pub volume: f32,
    /// Seconds between calls for a sound heard now and then (`AmbientRandom`); `None` for one that never stops.
    pub interval: Option<f32>,
    /// When it is heard (`AmbientSoundType`): by day, by night, or at any hour.
    pub heard: Heard,
    /// How fast it plays, 1 at the client's own `SoundPitch` of 64.
    pub pitch: f32,
}

/// The hours an ambient sound is heard in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// Birds, roosters and cicadas, which the maps mark 1.
    Day,
    /// Crickets, wolves and foxes, marked 2.
    Night,
    /// Water, fire, wind and drones, which carry no mark.
    Always,
}

/// Linear distance fog of a zone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    /// RGB; alpha is unused.
    pub color: [u8; 4],
    /// Distance where fog begins, in world units.
    pub start: f32,
    /// Distance where fog hides everything.
    pub end: f32,
}

/// Where a scene puts the camera and the fog of the zone it lands in.
#[derive(Debug, Clone, PartialEq)]
pub struct Warp {
    pub placement: Placement,
    pub fog: Option<Fog>,
    /// Name of the zone the camera lands in.
    pub zone: Option<String>,
    /// The zone's current time-of-day state, which picks terrain intensity maps.
    pub zone_state: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Level {
    pub actors: Vec<Actor>,
    /// Where each scene that starts with a warp puts the camera, by the scene's tag.
    pub warps: BTreeMap<String, Warp>,
    /// Every scene's camera shots, by the scene's tag.
    pub shots: BTreeMap<String, Vec<Shot>>,
    pub terrains: Vec<Terrain>,
    pub emitters: Vec<Emitter>,
    /// Sounds the level loops around a place, from its `AmbientSoundObject`s.
    pub ambient_sounds: Vec<AmbientSound>,
    /// The level's BSP polygons that draw.
    pub bsp: Vec<BspPolygon>,
    /// The fog of the level's own zone, which covers everything outside a zone of its own.
    pub fog: Option<Fog>,
}

/// Reads every placed actor, recognized by the `Level` reference the editor stores in each of them.
pub fn read_level(package: &Package, file: &[u8]) -> Result<Level, Error> {
    let mut level = Level::default();
    let mut scenes = Vec::new();
    for (index, export) in package.exports().iter().enumerate() {
        // Brushes keep empty models of their own; the level's is the one with polygons.
        if package.class_name(export).eq_ignore_ascii_case("Model") {
            if let Some(polygons) = bsp::read(package, file, export)?
                && polygons.len() > level.bsp.len()
            {
                level.bsp = polygons;
            }
            continue;
        }
        if export.serial_size == 0 || matches!(export.class, ObjectRef::Null) {
            continue;
        }
        let properties = object_properties(package, file, export)?;
        // The level itself is a zone: what it says covers every place no smaller zone claims.
        if package.class_name(export).eq_ignore_ascii_case("LevelInfo") {
            level.fog = fog(&properties);
        }
        // Ambient sounds are the one kind of actor the editor saves without the `Level` every other one carries.
        if package.class_name(export).eq_ignore_ascii_case("AmbientSoundObject") {
            if let Some(sound) = ambient_sound(package, &properties) {
                level.ambient_sounds.push(sound);
            }
            continue;
        }
        if find(&properties, "Level").is_none() {
            continue;
        }
        let actor = actor(package, file, index, &properties)?;
        if actor.class.eq_ignore_ascii_case("SceneManager") {
            scenes.push((actor.tag.clone(), properties));
        } else if actor.class.eq_ignore_ascii_case("TerrainInfo")
            && let Some(terrain) = terrain::read(package, file, index, &properties)
        {
            level.terrains.push(terrain);
        } else if actor.class.eq_ignore_ascii_case("Emitter") {
            let zone = zone(package, &properties).map(|zone| package.object_name(ObjectRef::Export(zone)).to_owned());
            level.emitters.push(emitters::read(package, file, &properties, zone)?);
        }
        level.actors.push(actor);
    }
    for (tag, properties) in scenes {
        let Some(tag) = tag else { continue };
        level.shots.insert(tag.clone(), shots::read(package, file, &properties)?);
        if let Some(warp) = first_warp(package, file, &properties)? {
            level.warps.insert(tag, warp);
        }
    }
    Ok(level)
}

fn actor(package: &Package, file: &[u8], export: usize, properties: &[Property<'_>]) -> Result<Actor, Error> {
    let get = |name: &str| find(properties, name);
    let path = |object: ObjectRef| (!matches!(object, ObjectRef::Null)).then(|| package.object_path(object));
    let draw_scale = get("DrawScale").and_then(Property::float).unwrap_or(1.0);
    let scale3d = get("DrawScale3D").and_then(Property::vector).unwrap_or([1.0; 3]);
    let exports = package.exports();
    let lighting = match get("StaticMeshInstance").and_then(|instance| instance.object(package)) {
        Some(ObjectRef::Export(instance)) => match exports.get(instance) {
            Some(instance) => lighting::vertex_colors(package, file, instance)?,
            None => Vec::new(),
        },
        _ => Vec::new(),
    };
    let class = exports.get(export).map(|export| package.class_name(export).to_owned()).unwrap_or_default();
    Ok(Actor {
        movement: movement::read(&class, properties, placement(properties)),
        class,
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
        unlit: get("bUnlit").and_then(Property::bool).unwrap_or(false),
        lighting,
    })
}

/// What an `AmbientSoundObject` plays, when it names a sound. Unreal keeps a sound's radius in twenty-five
/// world units, as `WorldSoundRadius` multiplies it, its volume as a byte, and its pitch as a byte where 64 plays
/// the sound as recorded.
fn ambient_sound(package: &Package, properties: &[Property<'_>]) -> Option<AmbientSound> {
    let sound = find(properties, "AmbientSound").and_then(|sound| sound.object(package))?;
    let byte = |name, default| find(properties, name).and_then(Property::byte).unwrap_or(default);
    let interval = find(properties, "AmbientRandom").and_then(Property::int).filter(|seconds| *seconds > 0);
    #[expect(clippy::cast_precision_loss, reason = "intervals are tens of seconds")]
    Some(AmbientSound {
        location: placement(properties).location,
        sound: (!matches!(sound, ObjectRef::Null)).then(|| package.object_path(sound))?,
        radius: find(properties, "SoundRadius").and_then(Property::float).unwrap_or(64.0) * 25.0,
        volume: f32::from(byte("SoundVolume", 255)) / 255.0,
        interval: interval.map(|seconds| seconds as f32),
        heard: match find(properties, "AmbientSoundType").and_then(Property::byte) {
            Some(1) => Heard::Day,
            Some(2) => Heard::Night,
            _ => Heard::Always,
        },
        pitch: f32::from(byte("SoundPitch", 64)) / 64.0,
    })
}

fn placement(properties: &[Property<'_>]) -> Placement {
    Placement {
        location: find(properties, "Location").and_then(Property::vector).unwrap_or_default(),
        rotation: find(properties, "Rotation").and_then(Property::rotator).unwrap_or_default(),
    }
}

/// The interpolation point the scene's first action warps to, if it starts with a warp.
fn first_warp(package: &Package, file: &[u8], scene: &[Property<'_>]) -> Result<Option<Warp>, Error> {
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
    let point = object_properties(package, file, point)?;
    // The zone an actor stands in is the first field of its Region.
    let zone = zone(package, &point);
    let (fog, zone_state) = match zone.and_then(|zone| package.exports().get(zone)) {
        Some(zone) => {
            let zone = object_properties(package, file, zone)?;
            let state = find(&zone, "CurZoneState");
            let state = state
                .and_then(Property::byte)
                .or_else(|| state.and_then(Property::int).and_then(|state| u8::try_from(state).ok()));
            (fog(&zone), state)
        }
        None => (None, None),
    };
    let zone = zone.map(|zone| package.object_name(ObjectRef::Export(zone)).to_owned());
    Ok(Some(Warp { placement: placement(&point), fog, zone, zone_state }))
}

/// The export index of the zone an actor stands in: the `Zone` field of its `Region`.
fn zone(package: &Package, actor: &[Property<'_>]) -> Option<usize> {
    let region = find(actor, "Region")?.fields(package)?;
    match find(&region, "Zone")?.object(package)? {
        ObjectRef::Export(index) => Some(index),
        _ => None,
    }
}

fn fog(zone: &[Property<'_>]) -> Option<Fog> {
    find(zone, "bDistanceFog").and_then(Property::bool).filter(|&fogged| fogged)?;
    let float = |name| find(zone, name).and_then(Property::float);
    // Unset values fall back to UT2004's ZoneInfo defaults.
    let color = find(zone, "DistanceFogColor").and_then(Property::color).unwrap_or([0, 0, 0, 255]);
    Some(Fog {
        color,
        start: float("DistanceFogStart").unwrap_or(3000.0),
        end: float("DistanceFogEnd").unwrap_or(8000.0),
    })
}

#[cfg(test)]
mod tests;

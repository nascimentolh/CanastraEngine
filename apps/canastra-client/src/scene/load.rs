//! Builds a map's static geometry as seen from one of its scene cameras: every static mesh placed in
//! world space relative to the camera, merged into one buffer range per material.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::rc::Rc;

use l2_catalog::{Catalog, Material, Mesh};
use ue2_assets::{Image, StaticMesh};
use ue2_level::{Actor, Emitter, Level, Placement, Shot, Warp};
use ue2_package::Package;

use super::daylight::Daylight;
use super::particle_mesh::{self, MeshKey, ParticleMesh};
use super::pipeline::draw_order;
use super::{bsp, camera, deco, sky, terrain};

/// The hour world zones are shown at. H5's lobby clock runs from 22:00 at six times real time; this is the hour
/// whose sky and light match the H5 creation screenshot.
// ponytail: a fixed hour; the running clock comes with the world.
const WORLD_HOUR: f32 = 21.0;

/// Position relative to the camera, UV, then an RGBA multiplier (white for level geometry).
pub(crate) type Vertex = [f32; 9];

pub(crate) struct Batch {
    pub(crate) material: Material,
    pub(crate) indices: std::ops::Range<u32>,
    /// Whether distance fog covers it; the sky stands beyond fog.
    pub(crate) fogged: bool,
}

pub(crate) struct SceneData {
    pub(crate) camera: Placement,
    /// The scene the camera was placed by, with the fog and zone it lands in.
    pub(crate) warp: Warp,
    /// Every scene's warp, by the scene's tag in lowercase.
    pub(crate) warps: BTreeMap<String, Warp>,
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u32>,
    /// Opaque batches first, then blended ones, in drawing order.
    pub(crate) batches: Vec<Batch>,
    /// Decoded textures by path, for every stage of every batch and every emitter sprite.
    pub(crate) textures: HashMap<String, Image>,
    /// The emitters of the zones the camera can warp to; each draws only while the camera is in its zone.
    pub(crate) emitters: Vec<Emitter>,
    /// The meshes mesh emitters draw, loaded with their materials' textures in `textures`.
    pub(crate) particle_meshes: HashMap<MeshKey, Rc<ParticleMesh>>,
    /// RGB multiplier of sprites that take the sky's color.
    pub(crate) cloud_tint: [f32; 3],
    /// The client's assets, kept to stand characters in the scene later.
    pub(crate) catalog: Catalog,
    /// The light on characters, in world zones.
    pub(crate) actor_daylight: Option<Daylight>,
    /// Every scene's camera shots, by the scene's tag in lowercase.
    pub(crate) shots: BTreeMap<String, Vec<Shot>>,
}

/// Geometry that draws with one material.
#[derive(Default)]
pub(super) struct Group {
    pub(super) vertices: Vec<Vertex>,
    pub(super) indices: Vec<u32>,
}

/// Loads `MAPS/<map>` from the client and frames it from the scene tagged `camera_tag`.
pub(crate) fn load(client_root: &Path, map: &str, camera_tag: &str) -> Result<SceneData, String> {
    let level = read_level(&client_root.join("MAPS").join(map))?;
    let warp = level.warps.get(camera_tag).cloned().ok_or_else(|| format!("{map} has no scene `{camera_tag}`"))?;
    let camera = warp.placement;
    let mut catalog = Catalog::open(client_root);
    let environment = l2_env::Environment::read(client_root);
    // Zones with states carry their light in the level; world zones are lit by the hour.
    let daylight_for = |sections| {
        environment
            .as_ref()
            .filter(|_| warp.zone_state.is_none())
            .and_then(|environment| Daylight::new(environment, WORLD_HOUR, &level.actors, sections))
    };
    let daylight = daylight_for(["StaticMeshAmbient", "HSVStaticMeshLight"]);
    let terrain_daylight = daylight_for(["TerrainAmbient", "HSVTerrainLight"]);
    let meshes = mesh_groups(&level, &mut catalog, camera.location, warp.zone_state, [&daylight, &terrain_daylight]);
    let bsp_daylight = daylight_for(["BSPAmbient", "HSVBSPLight"]);
    let actor_daylight = daylight_for(["ActorAmbient", "HSVActorLight"]);
    let brushes = bsp::groups(&level.bsp, &mut catalog, camera.location, bsp_daylight.as_ref());

    // The sky and terrain layers come first and in order: the stable sort below keeps their blending order.
    let sky = match &environment {
        Some(environment) if warp.zone_state.is_none() => sky::groups(&mut catalog, environment, WORLD_HOUR),
        _ => Vec::new(),
    };
    let mut groups: Vec<(Material, Group, bool)> = sky
        .into_iter()
        .map(|(material, group)| (material, group, false))
        .chain(
            terrain::groups(&level.terrains, &mut catalog, camera.location, warp.zone_state, terrain_daylight.as_ref())
                .into_iter()
                .chain(meshes)
                .chain(brushes)
                .map(|(material, group)| (material, group, true)),
        )
        .collect();
    // ponytail: blended batches draw by kind, not sorted by distance; sort them when overlaps show.
    groups.sort_by_key(|(material, _, _)| draw_order(material.blend));
    let mut data = SceneData {
        camera,
        warps: level.warps.iter().map(|(tag, warp)| (tag.to_ascii_lowercase(), warp.clone())).collect(),
        vertices: Vec::new(),
        indices: Vec::new(),
        batches: Vec::new(),
        textures: HashMap::new(),
        particle_meshes: HashMap::new(),
        emitters: level
            .emitters
            .into_iter()
            .filter_map(|emitter| scene_emitter(emitter, &level.warps, &warp))
            .collect(),
        // The login keeps the hour the client's clock starts at.
        // ponytail: SkyBoxColor, not a CloudColorN ramp, is the tint that matches the H5 login's haze by measurement; revisit with the world clock.
        cloud_tint: environment
            .and_then(|environment| environment.color("SkyBoxColor", environment.start_hour()))
            .map_or([1.0; 3], |color| color.map(|channel| f32::from(channel) / 255.0)),
        catalog: Catalog::default(),
        actor_daylight,
        shots: level.shots.iter().map(|(tag, shots)| (tag.to_ascii_lowercase(), shots.clone())).collect(),
        warp,
    };
    for texture in
        data.emitters.iter().flat_map(|emitter| &emitter.sprites).filter_map(|sprite| sprite.texture.as_ref())
    {
        decode(&mut data.textures, &mut catalog, texture);
    }
    for shape in data.emitters.iter().flat_map(|emitter| &emitter.sprites).filter_map(|sprite| sprite.mesh.as_ref()) {
        let key = particle_mesh::key(shape);
        if data.particle_meshes.contains_key(&key) {
            continue;
        }
        if let Some(mesh) = ParticleMesh::load(&mut catalog, shape) {
            for (material, _) in &mesh.sections {
                decode_material(&mut data.textures, &mut catalog, material);
            }
            data.particle_meshes.insert(key, Rc::new(mesh));
        }
    }
    for (material, group, fogged) in groups {
        decode_material(&mut data.textures, &mut catalog, &material);
        if !data.textures.contains_key(&material.base.texture) {
            continue;
        }
        let base = u32::try_from(data.vertices.len()).map_err(|_| "scene has too many vertices")?;
        let start = u32::try_from(data.indices.len()).map_err(|_| "scene has too many indices")?;
        data.indices.extend(group.indices.iter().map(|index| base + index));
        data.vertices.extend(group.vertices);
        let end = u32::try_from(data.indices.len()).map_err(|_| "scene has too many indices")?;
        data.batches.push(Batch { material, indices: start..end, fogged });
    }
    data.catalog = catalog;
    Ok(data)
}

/// `emitter` if it can show without loading the map again from `warp`, tagged with the zone it draws in. Zones no
/// scene warps into show nothing. An emitter in a zone whose scenes are all in another state, such as the light
/// beams inside the select hall that stand in the login's outdoor zone, joins the scene whose camera is nearest:
/// the BSP lets every zone see every other, so the zone alone does not say where it shows.
fn scene_emitter(mut emitter: Emitter, warps: &BTreeMap<String, Warp>, warp: &Warp) -> Option<Emitter> {
    let own: Vec<&Warp> = warps.values().filter(|other| other.zone == emitter.zone).collect();
    if own.iter().any(|other| other.zone_state == warp.zone_state) {
        return Some(emitter);
    }
    own.first()?;
    let distance = |other: &Warp| {
        other.placement.location.iter().zip(emitter.location).map(|(a, b)| (a - b) * (a - b)).sum::<f32>()
    };
    let nearest = warps.values().min_by(|a, b| distance(a).total_cmp(&distance(b)))?;
    emitter.zone.clone_from(&nearest.zone);
    (nearest.zone_state == warp.zone_state).then_some(emitter)
}

/// The static meshes of the level and its terrain decorations, placed relative to `camera` and merged into
/// one group per material.
fn mesh_groups(
    level: &Level,
    catalog: &mut Catalog,
    camera: [f32; 3],
    zone_state: Option<u8>,
    [daylight, terrain_daylight]: [&Option<Daylight>; 2],
) -> Vec<(Material, Group)> {
    let mut meshes: HashMap<String, Option<Mesh>> = HashMap::new();
    let mut materials: HashMap<String, Option<Material>> = HashMap::new();
    let mut groups: HashMap<String, Group> = HashMap::new();

    let decorations = deco::actors(&level.terrains, catalog, camera, zone_state, terrain_daylight.as_ref());
    // Decorations carry their full light already.
    let actors = level
        .actors
        .iter()
        .map(|actor| (actor, 1.0, daylight.as_ref()))
        .chain(decorations.iter().map(|(actor, opacity)| (actor, *opacity, None)));
    for (actor, opacity, daylight) in actors {
        let Some(path) = &actor.static_mesh else { continue };
        let mesh = meshes.entry(path.clone()).or_insert_with(|| catalog.static_mesh(path));
        let Some(Mesh { mesh, materials: slots }) = mesh.as_ref() else { continue };
        let axes = camera::axes(actor.placement.rotation);
        let relative = |position: [f32; 3]| {
            let mut world = camera::place(position, actor.scale, &axes, actor.placement.location);
            for (world, camera) in world.iter_mut().zip(camera) {
                *world -= camera;
            }
            world
        };
        // Where each of this actor's mesh vertices landed in each group, so shared vertices stay shared.
        let mut remaps: HashMap<String, HashMap<u16, u32>> = HashMap::new();
        for (slot, section) in mesh.sections.iter().enumerate() {
            let Some(path) = actor.skins.get(slot).or(slots.get(slot).and_then(Option::as_ref)) else { continue };
            // ponytail: sections whose material does not resolve are skipped.
            if materials.entry(path.clone()).or_insert_with(|| catalog.material(path)).is_none() {
                continue;
            }
            let group = groups.entry(path.clone()).or_default();
            let remap = remaps.entry(path.clone()).or_default();
            let first = section.first_index as usize;
            for &index in mesh.indices.get(first..first + section.triangles as usize * 3).unwrap_or_default() {
                let placed = *remap.entry(index).or_insert_with(|| {
                    let position = mesh.positions.get(usize::from(index)).copied().unwrap_or_default();
                    let at = relative(position);
                    let uv = mesh.uvs.get(usize::from(index)).copied().unwrap_or_default();
                    let light = vertex_light(actor, mesh, index, &axes, daylight);
                    group.vertices.push([at[0], at[1], at[2], uv[0], uv[1], light[0], light[1], light[2], opacity]);
                    u32::try_from(group.vertices.len() - 1).unwrap_or(u32::MAX)
                });
                group.indices.push(placed);
            }
        }
    }

    groups.into_iter().filter_map(|(path, group)| Some((materials.remove(&path)??, group))).collect()
}

/// The light a mesh vertex draws with: what the level stored for it, plus the hour's light in world zones.
// ponytail: actors without stored lighting (movers) draw unlit until dynamic lighting exists.
fn vertex_light(
    actor: &Actor,
    mesh: &StaticMesh,
    index: u16,
    axes: &[[f32; 3]; 3],
    daylight: Option<&Daylight>,
) -> [f32; 3] {
    let index = usize::from(index);
    if actor.unlit {
        return [1.0; 3];
    }
    let stored = actor
        .lighting
        .get(index)
        .map(|&[red, green, blue, _]| [red, green, blue].map(|channel| f32::from(channel) / 255.0));
    let (Some(daylight), Some(&normal)) = (daylight, mesh.normals.get(index)) else {
        return stored.unwrap_or([1.0; 3]);
    };
    // In world zones the hour lights every mesh, over whatever the level stored; trees store nothing.
    let lit = daylight.on_shaded(camera::place(normal, actor.scale.map(f32::signum), axes, [0.0; 3]), 1.0, 1.0);
    let mut light = stored.unwrap_or_default();
    for (light, lit) in light.iter_mut().zip(lit) {
        *light += lit;
    }
    light
}

/// Decodes every texture `material` draws with: its stages and, when its base animates, the other frames.
fn decode_material(textures: &mut HashMap<String, Image>, catalog: &mut Catalog, material: &Material) {
    let stages = std::iter::once(&material.base).chain(material.layer.as_ref().map(|(stage, _, _)| stage));
    for path in stages.map(|stage| &stage.texture).chain(&material.frames) {
        decode(textures, catalog, path);
    }
}

/// Decodes the texture at `path` into `textures` unless it is already there or cannot be read.
fn decode(textures: &mut HashMap<String, Image>, catalog: &mut Catalog, path: &str) {
    if !textures.contains_key(path)
        && let Some(image) = catalog.texture(path)
    {
        textures.insert(path.to_owned(), image);
    }
}

fn read_level(path: &Path) -> Result<Level, String> {
    let error = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
    let bytes = std::fs::read(path).map_err(|e| error(&e))?;
    let file = l2_crypto::decrypt(&bytes, path).map_err(|e| error(&e))?;
    let package = Package::parse(&file).map_err(|e| error(&e))?;
    ue2_level::read_level(&package, &file).map_err(|e| error(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_emitter_shows_with_its_zone_or_else_the_nearest_scene_in_this_state() {
        let warp = |x: f32, zone: &str, zone_state| Warp {
            placement: Placement { location: [x, 0.0, 0.0], rotation: [0; 3] },
            fog: None,
            zone: Some(zone.into()),
            zone_state,
        };
        let (login, select) = (warp(0.0, "Outdoor", Some(2)), warp(8000.0, "Hall", None));
        let warps = BTreeMap::from([("login".into(), login.clone()), ("select".into(), select.clone())]);
        let emitter = |x: f32, zone: &str| Emitter {
            location: [x, 0.0, 0.0],
            rotation: [0; 3],
            zone: Some(zone.into()),
            sprites: Vec::new(),
        };

        // A beam in the outdoor zone but inside the hall joins the select scene; the login keeps it in its zone.
        let beam = scene_emitter(emitter(7000.0, "Outdoor"), &warps, &select).map(|beam| beam.zone);
        assert_eq!(beam, Some(Some("Hall".into())));
        assert_eq!(
            scene_emitter(emitter(7000.0, "Outdoor"), &warps, &login).map(|beam| beam.zone),
            Some(Some("Outdoor".into()))
        );
        // Mist by the login camera does not show from the hall, nor anything in a zone no scene warps into.
        assert!(scene_emitter(emitter(100.0, "Outdoor"), &warps, &select).is_none());
        assert!(scene_emitter(emitter(7000.0, "Sky"), &warps, &select).is_none());
    }
}

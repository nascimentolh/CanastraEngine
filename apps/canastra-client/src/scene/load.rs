//! Builds a map's static geometry as seen from one of its scene cameras: every static mesh placed in
//! world space relative to the camera, merged into one buffer range per material.

use std::collections::HashMap;
use std::path::Path;

use l2_catalog::{Blend, Catalog, Material, Mesh};
use ue2_assets::Image;
use ue2_level::{Emitter, Fog, Level, Placement};
use ue2_package::Package;

use super::{camera, deco, terrain};

/// Position relative to the camera, UV, then an RGBA multiplier (white for level geometry).
pub(crate) type Vertex = [f32; 9];

pub(crate) struct Batch {
    pub(crate) material: Material,
    pub(crate) indices: std::ops::Range<u32>,
}

pub(crate) struct SceneData {
    pub(crate) camera: Placement,
    /// Distance fog of the zone the camera is in.
    pub(crate) fog: Option<Fog>,
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u32>,
    /// Opaque batches first, then blended ones, in drawing order.
    pub(crate) batches: Vec<Batch>,
    /// Decoded textures by path, for every stage of every batch and every emitter sprite.
    pub(crate) textures: HashMap<String, Image>,
    pub(crate) emitters: Vec<Emitter>,
    /// RGB multiplier of sprites that take the sky's color.
    pub(crate) cloud_tint: [f32; 3],
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
    let mut meshes: HashMap<String, Option<Mesh>> = HashMap::new();
    let mut materials: HashMap<String, Option<Material>> = HashMap::new();
    let mut groups: HashMap<String, Group> = HashMap::new();

    let decorations = deco::actors(&level.terrains, &mut catalog, camera.location, warp.zone_state);
    for actor in level.actors.iter().chain(&decorations) {
        let Some(path) = &actor.static_mesh else { continue };
        let mesh = meshes.entry(path.clone()).or_insert_with(|| catalog.static_mesh(path));
        let Some(Mesh { mesh, materials: slots }) = mesh.as_ref() else { continue };
        let axes = camera::axes(actor.placement.rotation);
        let relative = |position: [f32; 3]| {
            let mut world = camera::place(position, actor.scale, &axes, actor.placement.location);
            for (world, camera) in world.iter_mut().zip(camera.location) {
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
                    // ponytail: actors without stored lighting (movers) draw unlit until dynamic lighting exists.
                    let light = match actor.lighting.get(usize::from(index)) {
                        Some(&[red, green, blue, _]) if !actor.unlit => {
                            [red, green, blue].map(|channel| f32::from(channel) / 255.0)
                        }
                        _ => [1.0; 3],
                    };
                    group.vertices.push([at[0], at[1], at[2], uv[0], uv[1], light[0], light[1], light[2], 1.0]);
                    u32::try_from(group.vertices.len() - 1).unwrap_or(u32::MAX)
                });
                group.indices.push(placed);
            }
        }
    }

    // Terrain layers come first and in order: the stable sort below keeps their blending order.
    let mut groups: Vec<(Material, Group)> =
        terrain::groups(&level.terrains, &mut catalog, camera.location, warp.zone_state)
            .into_iter()
            .chain(groups.into_iter().filter_map(|(path, group)| Some((materials.remove(&path)??, group))))
            .collect();
    // ponytail: blended batches draw by kind, not sorted by distance; sort them when overlaps show.
    groups.sort_by_key(|(material, _)| match material.blend {
        Blend::Opaque | Blend::Masked => 0,
        Blend::Alpha | Blend::AlphaAdditive => 1,
        Blend::Modulate | Blend::Brighten | Blend::Translucent | Blend::Darken => 2,
    });
    let mut data = SceneData {
        camera,
        fog: warp.fog,
        vertices: Vec::new(),
        indices: Vec::new(),
        batches: Vec::new(),
        textures: HashMap::new(),
        // Other zones are closed off from the camera's; only its own emitters can be seen.
        // ponytail: zones stand in for BSP portal visibility; add portals when a scene looks into another zone.
        emitters: level.emitters.into_iter().filter(|emitter| emitter.zone == warp.zone).collect(),
        // The login keeps the hour the client's clock starts at.
        // ponytail: SkyBoxColor, not a CloudColorN ramp, is the tint that matches the H5 login's haze by measurement; revisit with the world clock.
        cloud_tint: l2_env::Environment::read(client_root)
            .and_then(|environment| environment.color("SkyBoxColor", environment.start_hour()))
            .map_or([1.0; 3], |color| color.map(|channel| f32::from(channel) / 255.0)),
    };
    for texture in
        data.emitters.iter().flat_map(|emitter| &emitter.sprites).filter_map(|sprite| sprite.texture.as_ref())
    {
        if !data.textures.contains_key(texture)
            && let Some(image) = catalog.texture(texture)
        {
            data.textures.insert(texture.clone(), image);
        }
    }
    for (material, group) in groups {
        let stages = std::iter::once(&material.base).chain(material.layer.as_ref().map(|(stage, _, _)| stage));
        for stage in stages {
            if !data.textures.contains_key(&stage.texture)
                && let Some(image) = catalog.texture(&stage.texture)
            {
                data.textures.insert(stage.texture.clone(), image);
            }
        }
        if !data.textures.contains_key(&material.base.texture) {
            continue;
        }
        let base = u32::try_from(data.vertices.len()).map_err(|_| "scene has too many vertices")?;
        let start = u32::try_from(data.indices.len()).map_err(|_| "scene has too many indices")?;
        data.indices.extend(group.indices.iter().map(|index| base + index));
        data.vertices.extend(group.vertices);
        let end = u32::try_from(data.indices.len()).map_err(|_| "scene has too many indices")?;
        data.batches.push(Batch { material, indices: start..end });
    }
    Ok(data)
}

fn read_level(path: &Path) -> Result<Level, String> {
    let error = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
    let bytes = std::fs::read(path).map_err(|e| error(&e))?;
    let file = l2_crypto::decrypt(&bytes, path).map_err(|e| error(&e))?;
    let package = Package::parse(&file).map_err(|e| error(&e))?;
    ue2_level::read_level(&package, &file).map_err(|e| error(&e))
}

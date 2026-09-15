//! Builds a map's static geometry as seen from one of its scene cameras: every static mesh placed in
//! world space relative to the camera, merged into one buffer per texture.

use std::collections::HashMap;
use std::path::Path;

use l2_catalog::{Catalog, Mesh};
use ue2_assets::Image;
use ue2_level::{Level, Placement};
use ue2_package::Package;

use super::camera;

/// Position relative to the camera, then UV.
pub(crate) type Vertex = [f32; 5];

pub(crate) struct Batch {
    pub(crate) texture: Image,
    pub(crate) indices: std::ops::Range<u32>,
}

pub(crate) struct SceneData {
    pub(crate) camera: Placement,
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u32>,
    pub(crate) batches: Vec<Batch>,
}

/// Geometry that draws with one texture.
#[derive(Default)]
struct Group {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

/// Loads `MAPS/<map>` from the client and frames it from the scene tagged `camera_tag`.
pub(crate) fn load(client_root: &Path, map: &str, camera_tag: &str) -> Result<SceneData, String> {
    let level = read_level(&client_root.join("MAPS").join(map))?;
    let camera = *level.warps.get(camera_tag).ok_or_else(|| format!("{map} has no scene `{camera_tag}`"))?;
    let mut catalog = Catalog::open(client_root);
    let mut meshes: HashMap<String, Option<Mesh>> = HashMap::new();
    let mut textures: HashMap<String, Option<String>> = HashMap::new();
    let mut groups: HashMap<String, Group> = HashMap::new();

    for actor in &level.actors {
        let Some(path) = &actor.static_mesh else { continue };
        let mesh = meshes.entry(path.clone()).or_insert_with(|| catalog.static_mesh(path));
        let Some(Mesh { mesh, materials }) = mesh.as_ref() else { continue };
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
            let material = actor.skins.get(slot).or(materials.get(slot).and_then(Option::as_ref));
            let Some(material) = material else { continue };
            let texture = textures.entry(material.clone()).or_insert_with(|| catalog.material_texture(material));
            // ponytail: sections without a resolvable base texture are skipped; material support replaces this.
            let Some(texture) = texture.clone() else { continue };
            let group = groups.entry(texture.clone()).or_default();
            let remap = remaps.entry(texture).or_default();
            let first = section.first_index as usize;
            for &index in mesh.indices.get(first..first + section.triangles as usize * 3).unwrap_or_default() {
                let placed = *remap.entry(index).or_insert_with(|| {
                    let position = mesh.positions.get(usize::from(index)).copied().unwrap_or_default();
                    let at = relative(position);
                    let uv = mesh.uvs.get(usize::from(index)).copied().unwrap_or_default();
                    group.vertices.push([at[0], at[1], at[2], uv[0], uv[1]]);
                    u32::try_from(group.vertices.len() - 1).unwrap_or(u32::MAX)
                });
                group.indices.push(placed);
            }
        }
    }

    let mut data = SceneData { camera, vertices: Vec::new(), indices: Vec::new(), batches: Vec::new() };
    for (path, group) in groups {
        let Some(texture) = catalog.texture(&path) else { continue };
        let base = u32::try_from(data.vertices.len()).map_err(|_| "scene has too many vertices")?;
        let start = u32::try_from(data.indices.len()).map_err(|_| "scene has too many indices")?;
        data.indices.extend(group.indices.iter().map(|index| base + index));
        data.vertices.extend(group.vertices);
        let end = u32::try_from(data.indices.len()).map_err(|_| "scene has too many indices")?;
        data.batches.push(Batch { texture, indices: start..end });
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

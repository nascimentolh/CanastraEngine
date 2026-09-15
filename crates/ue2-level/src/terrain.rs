//! `TerrainInfo` actors: a heightmap placed and scaled in the world, painted by blended layers.

use ue2_assets::{Property, find};
use ue2_package::{ObjectRef, Package};

use crate::Range;
use crate::emitters::{range_of, range_vector};
use crate::lighting::{self, TerrainSector};

#[derive(Debug, Clone, PartialEq)]
pub struct Terrain {
    /// World position of the heightmap's center.
    pub location: [f32; 3],
    /// World units per heightmap sample on X and Y, and per 256 height steps on Z.
    pub scale: [f32; 3],
    /// Object path of the 16-bit heightmap texture.
    pub heightmap: String,
    /// Painted layers, bottom first.
    pub layers: Vec<TerrainLayer>,
    /// One bit per quad, row by row from the lowest bit of the first word; a clear bit is a hole.
    pub visible_quads: Vec<u32>,
    /// Precomputed lighting, patch by patch.
    pub sectors: Vec<TerrainSector>,
    /// Static meshes scattered over the terrain, such as grass.
    pub deco_layers: Vec<DecoLayer>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerrainLayer {
    /// Object path of the material the layer paints with.
    pub material: String,
    /// Object path of the texture whose red channel weighs the layer; `None` paints everywhere.
    pub alpha_map: Option<String>,
    /// Heightmap samples covered by one repeat of the material.
    pub scale: [f32; 2],
}

/// A static mesh scattered over the quads a density map paints.
#[derive(Debug, Clone, PartialEq)]
pub struct DecoLayer {
    /// Object path of the scattered static mesh.
    pub static_mesh: String,
    /// Object path of the texture whose red channel weighs how many land on each quad.
    pub density_map: String,
    /// Multiplies the density map, picked per placement.
    pub density_multiplier: Range,
    pub max_per_quad: u32,
    /// Scale on X, Y and Z.
    pub scale: [Range; 3],
    /// Distance from the camera where decorations begin and finish fading out.
    pub fadeout_radius: Range,
    pub seed: i32,
    pub random_yaw: bool,
    /// Also placed on quads that are holes.
    pub on_invisible_terrain: bool,
}

pub(crate) fn read(package: &Package, file: &[u8], export: usize, properties: &[Property<'_>]) -> Option<Terrain> {
    let path = |property: &Property<'_>| match property.object(package)? {
        ObjectRef::Null => None,
        object => Some(package.object_path(object)),
    };
    let layers = properties
        .iter()
        .filter(|property| property.name.eq_ignore_ascii_case("Layers"))
        .filter_map(|layer| {
            let fields = layer.fields(package)?;
            let float = |name| find(&fields, name).and_then(Property::float).unwrap_or(1.0);
            Some(TerrainLayer {
                material: find(&fields, "Texture").and_then(path)?,
                alpha_map: find(&fields, "AlphaMap").and_then(path),
                scale: [float("UScale"), float("VScale")],
            })
        })
        .collect();
    Some(Terrain {
        location: find(properties, "Location").and_then(Property::vector).unwrap_or_default(),
        scale: find(properties, "TerrainScale").and_then(Property::vector)?,
        heightmap: find(properties, "TerrainMap").and_then(path)?,
        layers,
        visible_quads: find(properties, "QuadVisibilityBitmap")
            .and_then(Property::ints)
            .map(|words| words.into_iter().map(i32::cast_unsigned).collect())
            .unwrap_or_default(),
        // Unlike Layers, a dynamic array.
        deco_layers: find(properties, "DecoLayers")
            .and_then(|layers| layers.structs(package))
            .unwrap_or_default()
            .iter()
            .filter_map(|layer| deco_layer(package, layer, path))
            .collect(),
        // Sectors are objects inside the terrain actor.
        sectors: package
            .exports()
            .iter()
            .filter(|sector| {
                sector.outer == ObjectRef::Export(export)
                    && package.class_name(sector).eq_ignore_ascii_case("TerrainSector")
            })
            .filter_map(|sector| lighting::sector(package, file, sector))
            .collect(),
    })
}

fn deco_layer(
    package: &Package,
    fields: &[Property<'_>],
    path: impl Fn(&Property<'_>) -> Option<String>,
) -> Option<DecoLayer> {
    // The editor stores these flags as integers.
    let int = |name| find(fields, name).and_then(|field| field.int().or_else(|| field.byte().map(i32::from)));
    if int("ShowOnTerrain") == Some(0) {
        return None;
    }
    Some(DecoLayer {
        static_mesh: find(fields, "StaticMesh").and_then(&path)?,
        density_map: find(fields, "DensityMap").and_then(&path)?,
        density_multiplier: find(fields, "DensityMultiplier").map_or([1.0; 2], |range| range_of(package, range)),
        max_per_quad: u32::try_from(int("MaxPerQuad").unwrap_or(0)).unwrap_or(0),
        scale: find(fields, "ScaleMultiplier").map_or([[1.0; 2]; 3], |ranges| range_vector(package, ranges)),
        fadeout_radius: find(fields, "FadeoutRadius").map_or([0.0; 2], |range| range_of(package, range)),
        seed: int("Seed").unwrap_or(0),
        random_yaw: int("RandomYaw").unwrap_or(0) != 0,
        on_invisible_terrain: int("ShowOnInvisibleTerrain").unwrap_or(0) != 0,
    })
}

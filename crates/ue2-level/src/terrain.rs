//! `TerrainInfo` actors: a heightmap placed and scaled in the world, painted by blended layers.

use ue2_assets::{Property, find};
use ue2_package::{ObjectRef, Package};

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

pub(crate) fn read(package: &Package, properties: &[Property<'_>]) -> Option<Terrain> {
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
    })
}

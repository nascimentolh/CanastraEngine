//! Precomputed lighting stored with a level: vertex colors of each static mesh actor and intensity
//! maps of each terrain sector.

use ue2_assets::{Error, object_data};
use ue2_core::Reader;
use ue2_package::{Export, Package};

/// A terrain sector: the patch of heightmap quads it covers and its intensity maps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainSector {
    /// First quad on X and Y.
    pub offset: [u32; 2],
    /// Quads on X and Y; the sector has one more vertex than quads on each axis.
    pub quads: [u32; 2],
    /// One intensity per vertex, row by row, for each of the level's time-of-day states.
    pub intensities: Vec<Vec<u8>>,
}

/// A `StaticMeshInstance`'s lighting: one color per vertex of the actor's mesh.
pub(crate) fn vertex_colors(package: &Package, file: &[u8], instance: &Export) -> Result<Vec<[u8; 4]>, Error> {
    let (_, data) = object_data(package, file, instance)?;
    let mut reader = Reader::at(data, 0);
    let count = usize::try_from(reader.compact()?).unwrap_or(0);
    // Serialized as red, green, blue, alpha, unlike Color struct properties.
    let colors = reader.bytes(count.checked_mul(4).ok_or(Error::BadMesh)?)?;
    Ok(colors.as_chunks::<4>().0.to_vec())
}

/// A `TerrainSector`'s placement and intensity maps, or `None` when its layout is not recognized.
pub(crate) fn sector(package: &Package, file: &[u8], export: &Export) -> Option<TerrainSector> {
    let (_, data) = object_data(package, file, export).ok()?;
    let mut reader = Reader::at(data, 1);
    let quads = [reader.u32().ok()?, reader.u32().ok()?];
    let offset = [reader.u32().ok()?, reader.u32().ok()?];
    let vertices = usize::try_from((quads[0] + 1) * (quads[1] + 1)).ok()?;
    // Lineage 2 stores variable data between the bounds and the maps, so the maps are found by their
    // own shape: a count, then that many byte arrays of one entry per vertex.
    let intensities = (reader.pos()..data.len()).find_map(|start| maps_at(data, start, vertices))?;
    Some(TerrainSector { offset, quads, intensities })
}

fn maps_at(data: &[u8], start: usize, vertices: usize) -> Option<Vec<Vec<u8>>> {
    let mut reader = Reader::at(data, start);
    let count = reader.u32().ok()?;
    if !(1..=16).contains(&count) {
        return None;
    }
    (0..count)
        .map(|_| {
            let len = usize::try_from(reader.compact().ok()?).ok()?;
            (len == vertices).then(|| reader.bytes(len).ok().map(<[u8]>::to_vec)).flatten()
        })
        .collect()
}

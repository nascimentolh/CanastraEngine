//! A level's BSP (its `Model`): the walls, floors and other brush geometry, as polygons with the material
//! and texture axes of the surface each lies on.

use ue2_assets::{Error, object_data};
use ue2_core::Reader;
use ue2_package::{Export, ObjectRef, Package};

/// Polygon flags whose surfaces are never drawn: invisible, portal, sky backdrop, and one Lineage II uses
/// for hidden surfaces.
const HIDDEN: u32 = 0x0000_0001 | 0x0400_0000 | 0x0000_0080 | 0x0000_2000;

#[derive(Debug, Clone, PartialEq)]
pub struct BspPolygon {
    /// Object path of the material, when set.
    pub material: Option<String>,
    /// Convex polygon corners in world space, fanned from the first.
    pub corners: Vec<[f32; 3]>,
    /// Where texture coordinates are zero, and the texture's U and V axes; a corner's texel coordinate on
    /// an axis is its offset from the origin dotted with that axis.
    pub texture_origin: [f32; 3],
    pub texture_axes: [[f32; 3]; 2],
}

/// References are kept as stored: -1 means none.
struct Node {
    first_vertex: i32,
    surface: i32,
    vertices: usize,
}

struct Surface {
    material: ObjectRef,
    flags: u32,
    origin: i32,
    axes: [i32; 2],
}

/// The drawable polygons of a `Model` export; `None` when it holds no geometry, as brush models do.
pub(crate) fn read(package: &Package, file: &[u8], export: &Export) -> Result<Option<Vec<BspPolygon>>, Error> {
    let (_, data) = object_data(package, file, export)?;
    let mut reader = Reader::at(data, 0);
    let reader = &mut reader;
    reader.bytes(25 + 16)?; // bounding box and sphere
    let vectors = array(reader, vector)?;
    let points = array(reader, vector)?;
    let nodes = array(reader, |reader| {
        reader.bytes(16 + 8 + 1)?; // plane, zone mask and flags
        let first_vertex = reader.compact()?;
        let surface = reader.compact()?;
        for _ in 0..5 {
            reader.compact()?; // back, front, coplanar, collision bound and render bound
        }
        reader.bytes(32)?; // bounding spheres
        reader.bytes(2)?; // back and front zones
        let vertices = usize::from(reader.u8()?);
        reader.bytes(4 * 5)?; // leaves, section, first vertex and light map
        Ok(Node { first_vertex, surface, vertices })
    })?;
    let lineage = package.licensee >= 23;
    let surfaces = array(reader, |reader| {
        let material = package.object_at(reader.compact()?)?;
        let flags = reader.u32()?;
        let origin = reader.compact()?;
        reader.compact()?; // normal
        let axes = [reader.compact()?, reader.compact()?];
        reader.compact()?; // brush polygon
        reader.compact()?; // brush actor
        reader.bytes(16 + 4)?; // plane and light map scale
        if lineage {
            reader.i32()?;
        }
        Ok(Surface { material, flags, origin, axes })
    })?;
    let vertices = array(reader, |reader| {
        let point = reader.compact()?;
        reader.compact()?; // side
        Ok(point)
    })?;
    if nodes.is_empty() {
        return Ok(None);
    }

    // A polygon whose references lead nowhere is left out rather than failing the level.
    let polygons = nodes
        .iter()
        .filter(|node| node.vertices >= 3)
        .filter_map(|node| {
            let surface = get(&surfaces, node.surface)?;
            if surface.flags & HIDDEN != 0 {
                return None;
            }
            let first = usize::try_from(node.first_vertex).ok()?;
            let corners = vertices
                .get(first..first + node.vertices)?
                .iter()
                .map(|&point| get(&points, point).copied())
                .collect::<Option<_>>()?;
            Some(BspPolygon {
                material: (!matches!(surface.material, ObjectRef::Null)).then(|| package.object_path(surface.material)),
                corners,
                texture_origin: *get(&points, surface.origin)?,
                texture_axes: [*get(&vectors, surface.axes[0])?, *get(&vectors, surface.axes[1])?],
            })
        })
        .collect();
    Ok(Some(polygons))
}

fn get<T>(items: &[T], index: i32) -> Option<&T> {
    items.get(usize::try_from(index).ok()?)
}

fn vector(reader: &mut Reader<'_>) -> Result<[f32; 3], Error> {
    Ok([reader.f32()?, reader.f32()?, reader.f32()?])
}

fn array<'a, T>(
    reader: &mut Reader<'a>,
    mut item: impl FnMut(&mut Reader<'a>) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let len = usize::try_from(reader.compact()?).map_err(|_| Error::BadMesh)?;
    if len > reader.remaining().len() {
        return Err(Error::BadMesh);
    }
    (0..len).map(|_| item(reader)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floats(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    /// A model with a triangle, a node whose surface is missing, and one surface.
    fn model() -> Vec<u8> {
        let mut data = vec![0]; // properties: None
        data.extend([0; 41]);
        data.push(2);
        data.extend(floats(&[0.5, 0.0, 0.0, 0.0, 0.25, 0.0]));
        data.push(4);
        data.extend(floats(&[10.0, 0.0, 0.0, 12.0, 0.0, 0.0, 10.0, 4.0, 0.0, 10.0, 0.0, 0.0]));
        data.push(2);
        for surface in [0, 0x81] {
            data.extend([0; 25]);
            data.extend([0, surface, 0, 0, 0, 0, 0]);
            data.extend([0; 32]);
            data.extend([0, 3, 3]);
            data.extend([0; 20]);
        }
        data.push(1);
        data.extend([0, 0, 0, 0, 0, 3, 0, 0, 1, 0, 0]);
        data.extend([0; 20]);
        data.push(3);
        data.extend([0, 0, 1, 0, 2, 0]);
        data
    }

    fn package(object: &[u8]) -> Vec<u8> {
        let mut file = Vec::new();
        for value in [0x9E2A_83C1, 123, 0, 1, 36, 1, 0, 0, 0] {
            file.extend(u32::to_le_bytes(value));
        }
        file.extend([5, b'N', b'o', b'n', b'e', 0, 0, 0, 0, 0]);
        let export_offset = u32::try_from(file.len()).unwrap();
        let (offset, size) = (file.len() + 15, object.len());
        file[24..28].copy_from_slice(&export_offset.to_le_bytes());
        file.extend([0; 11]);
        for value in [size, offset] {
            file.push(u8::try_from(value & 0x3F).unwrap() | 0x40);
            file.push(u8::try_from(value >> 6).unwrap());
        }
        file.extend(object);
        file
    }

    #[test]
    #[expect(clippy::float_cmp, reason = "values are written and read back as the same bits")]
    fn reads_polygons_and_leaves_out_broken_references() {
        let file = package(&model());
        let package = Package::parse(&file).unwrap();
        let polygons = read(&package, &file, &package.exports()[0]).unwrap().unwrap();

        assert_eq!(polygons.len(), 1);
        let polygon = &polygons[0];
        assert_eq!(polygon.corners, [[10.0, 0.0, 0.0], [12.0, 0.0, 0.0], [10.0, 4.0, 0.0]]);
        assert_eq!(polygon.texture_origin, [10.0, 0.0, 0.0]);
        assert_eq!(polygon.texture_axes, [[0.5, 0.0, 0.0], [0.0, 0.25, 0.0]]);
    }
}

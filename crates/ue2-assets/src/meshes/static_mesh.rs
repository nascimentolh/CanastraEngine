//! `StaticMesh` objects: the vertex, UV and index streams a renderer needs, plus section materials.

use ue2_package::{Export, ObjectRef, Package};

use super::{array, count, vector};
use crate::Error;
use crate::properties::{Property, find, object_properties_reader};

/// A run of triangles drawn with one material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    /// Offset into `StaticMesh::indices`.
    pub first_index: u32,
    pub triangles: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticMesh {
    pub positions: Vec<[f32; 3]>,
    /// First UV channel; empty when the mesh has none.
    pub uvs: Vec<[f32; 2]>,
    /// Triangle list indices into the vertex streams.
    pub indices: Vec<u16>,
    pub sections: Vec<Section>,
    /// Material of each section, in section order; `Null` when unset.
    pub materials: Vec<ObjectRef>,
}

pub fn read_static_mesh(package: &Package, file: &[u8], export: &Export) -> Result<StaticMesh, Error> {
    let (properties, mut reader) = object_properties_reader(package, file, export)?;
    let materials = materials(package, find(&properties, "Materials"));

    reader.bytes(25 + 16)?; // bounding box and sphere
    let sections = array(&mut reader, |reader| {
        reader.u32()?; // strip flag: sections here are always triangle lists
        let first_index = u32::from(reader.u16()?);
        reader.bytes(4)?; // vertex index range
        let triangles = u32::from(reader.u16()?);
        reader.u16()?; // primitive count
        Ok(Section { first_index, triangles })
    })?;
    reader.bytes(25)?; // bounding box again

    // ponytail: normals are skipped until the scene is lit.
    let positions = array(&mut reader, |reader| {
        let position = vector(reader)?;
        reader.bytes(12)?; // normal
        Ok(position)
    })?;
    reader.u32()?; // revision
    for _ in 0..2 {
        // Color and alpha streams; lighting comes from the level, not from here.
        let len = count(&mut reader)?;
        reader.bytes(len.checked_mul(4).ok_or(Error::BadMesh)?)?;
        reader.u32()?;
    }
    let mut uv_streams = array(&mut reader, |reader| {
        let uvs = array(reader, |reader| Ok([reader.f32()?, reader.f32()?]))?;
        reader.bytes(8)?; // coordinate index and revision
        Ok(uvs)
    })?;
    let indices = array(&mut reader, |reader| Ok(reader.u16()?))?;

    let uvs = if uv_streams.is_empty() { Vec::new() } else { uv_streams.swap_remove(0) };
    let mesh = StaticMesh { positions, uvs, indices, sections, materials };
    validate(&mesh)?;
    Ok(mesh)
}

/// Every index and section must stay inside the streams, which a misread layout cannot satisfy.
fn validate(mesh: &StaticMesh) -> Result<(), Error> {
    let vertices = mesh.positions.len();
    let uvs_fit = mesh.uvs.is_empty() || mesh.uvs.len() == vertices;
    let indices_fit = mesh.indices.iter().all(|&index| usize::from(index) < vertices);
    let sections_fit = mesh.sections.iter().all(|section| {
        (section.first_index as usize)
            .checked_add(section.triangles as usize * 3)
            .is_some_and(|end| end <= mesh.indices.len())
    });
    if uvs_fit && indices_fit && sections_fit { Ok(()) } else { Err(Error::BadMesh) }
}

/// The `Material` of each `StaticMeshMaterial` in the `Materials` property.
fn materials(package: &Package, property: Option<&Property<'_>>) -> Vec<ObjectRef> {
    property
        .and_then(|materials| materials.structs(package))
        .map(|materials| {
            materials
                .iter()
                .map(|material| {
                    find(material, "Material").and_then(|material| material.object(package)).unwrap_or(ObjectRef::Null)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f32s(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    /// A package whose only export is a quad with one section, laid out like `L2_Lobby.BloodLobbyMoon_S`.
    fn quad(indices: &[u16]) -> Vec<u8> {
        let mut mesh = vec![0]; // properties: None
        mesh.extend([0; 41]); // bounding box and sphere
        mesh.push(1); // one section
        mesh.extend([0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 2, 0, 2, 0]);
        mesh.extend([0; 25]);
        mesh.push(4);
        for corner in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            mesh.extend(f32s(&[corner[0], 0.0, corner[1], 0.0, -1.0, 0.0]));
        }
        mesh.extend([2, 0, 0, 0]);
        for _ in 0..2 {
            mesh.push(4);
            mesh.extend([255; 16]);
            mesh.extend([2, 0, 0, 0]);
        }
        mesh.extend([1, 4]);
        mesh.extend(f32s(&[0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]));
        mesh.extend([0; 8]);
        mesh.push(u8::try_from(indices.len()).unwrap());
        mesh.extend(indices.iter().flat_map(|index| index.to_le_bytes()));
        mesh.extend([2, 0, 0, 0]);

        let mut file = Vec::new();
        for value in [0x9E2A_83C1, 123, 0, 1, 36, 1, 0, 0, 0] {
            file.extend(u32::to_le_bytes(value));
        }
        file.extend([5, b'N', b'o', b'n', b'e', 0, 0, 0, 0, 0]);
        let export_offset = u32::try_from(file.len()).unwrap();
        // The export entry is 11 bytes of fixed fields plus two 2-byte compact indices.
        let (offset, size) = (file.len() + 15, mesh.len());
        file[24..28].copy_from_slice(&export_offset.to_le_bytes());
        file.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // class, super, outer, name, flags
        for value in [size, offset] {
            file.push(u8::try_from(value & 0x3F).unwrap() | 0x40);
            file.push(u8::try_from(value >> 6).unwrap());
        }
        file.extend(mesh);
        file
    }

    #[test]
    #[expect(clippy::float_cmp, reason = "values are written and read back as the same bits")]
    fn reads_streams_and_rejects_indices_outside_them() {
        let file = quad(&[0, 1, 2, 2, 3, 0]);
        let package = Package::parse(&file).unwrap();
        let mesh = read_static_mesh(&package, &file, &package.exports()[0]).unwrap();
        assert_eq!(mesh.positions[2], [1.0, 0.0, 1.0]);
        assert_eq!(mesh.uvs[1], [1.0, 1.0]);
        assert_eq!(mesh.indices, [0, 1, 2, 2, 3, 0]);
        assert_eq!(mesh.sections, [Section { first_index: 0, triangles: 2 }]);

        let file = quad(&[0, 1, 2, 2, 3, 4]);
        let package = Package::parse(&file).unwrap();
        assert!(matches!(read_static_mesh(&package, &file, &package.exports()[0]), Err(Error::BadMesh)));
    }
}

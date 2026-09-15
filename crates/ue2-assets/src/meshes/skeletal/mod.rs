//! `SkeletalMesh` objects as Lineage II High Five stores them: a skinned body part with its bind skeleton.
//!
//! Only the first level of detail is kept.

mod lod;

use ue2_core::Reader;
use ue2_package::{Export, ObjectRef, Package};

use super::{Section, array, count, skip_array, vector};
use crate::Error;
use crate::properties::object_properties_reader;

/// The layout these readers follow: package version 123, licensee 37.
const LAYOUT: (u16, u16) = (123, 37);

/// Refuses packages saved with another layout, such as custom content made with other tools.
pub(super) fn check_layout(package: &Package) -> Result<(), Error> {
    if (package.version, package.licensee) == LAYOUT {
        Ok(())
    } else {
        Err(Error::UnsupportedLayout { version: package.version, licensee: package.licensee })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bone {
    pub name: String,
    /// Index of the parent bone; the root is its own parent.
    pub parent: usize,
    /// Bind pose relative to the parent: rotation as a quaternion (x, y, z, w), then translation.
    pub rotation: [f32; 4],
    pub position: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Indices into `SkeletalMesh::bones`; unused slots have weight 0.
    pub bones: [u16; 4],
    pub weights: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkeletalMesh {
    pub bones: Vec<Bone>,
    pub vertices: Vec<SkinVertex>,
    /// Triangle list indices into `vertices`.
    pub indices: Vec<u16>,
    pub sections: Vec<Section>,
    /// Material of each section, in section order; `Null` when unset.
    pub materials: Vec<ObjectRef>,
    /// The `MeshAnimation` the mesh plays by default.
    pub animation: ObjectRef,
    pub scale: [f32; 3],
    pub origin: [f32; 3],
    /// Pitch, yaw and roll in Unreal rotation units.
    pub rotation: [i32; 3],
}

pub fn read_skeletal_mesh(package: &Package, file: &[u8], export: &Export) -> Result<SkeletalMesh, Error> {
    check_layout(package)?;
    let (_, mut reader) = object_properties_reader(package, file, export)?;
    let reader = &mut reader;
    reader.bytes(25 + 16)?; // bounding box and sphere

    // LodMesh
    let version = reader.i32()?;
    reader.i32()?; // vertex count
    skip_array(reader, 4)?; // packed vertices of the older mesh format
    let textures = array(reader, |reader| Ok(package.object_at(reader.compact()?)?))?;
    let scale = vector(reader)?;
    let origin = vector(reader)?;
    let rotation = [reader.i32()?, reader.i32()?, reader.i32()?];
    skip_array(reader, 2)?; // face levels
    skip_array(reader, 8)?; // faces
    skip_array(reader, 2)?; // collapse wedges
    skip_array(reader, 10)?; // wedges
    let slots = array(reader, |reader| {
        reader.u32()?; // poly flags
        Ok(reader.i32()?)
    })?;
    reader.bytes(4 * 6)?; // scale max, hysteresis, strength, min vertices, morph, z displace
    if version >= 3 {
        reader.bytes(4)?; // has impostor
        reader.compact()?; // sprite material
        reader.bytes(12 + 12 + 12 + 4 + 12)?; // impostor location, rotation, scale, color and modes
    }
    if version >= 4 {
        reader.f32()?; // skin tessellation factor
    }
    if version >= 5 {
        reader.i32()?;
    }
    if version >= 6 {
        reader.u8()?;
    }

    // SkeletalMesh
    skip_array(reader, 12)?; // points of the older mesh format
    let bones = array(reader, |reader| bone(package, reader))?;
    let animation = package.object_at(reader.compact()?)?;
    reader.i32()?; // skeletal depth
    array(reader, |reader| {
        skip_array(reader, 2)?;
        Ok(reader.i32()?)
    })?; // weight indices
    skip_array(reader, 4)?; // bone influences
    array(reader, |reader| Ok(reader.compact()?))?; // attachment aliases
    array(reader, |reader| Ok(reader.compact()?))?; // attachment bone names
    skip_array(reader, 48)?; // attachment coordinates
    let extra_lods = count(reader)?.checked_sub(1).ok_or(Error::BadMesh)?;
    let lod = lod::read(reader, bones.len())?;
    for _ in 0..extra_lods {
        lod::read(reader, bones.len())?;
    }
    reader.compact()?;
    for size in [12, 10, 12, 8, 2, 2] {
        reader.u32()?; // lazy array skip offset
        skip_array(reader, size)?; // points, wedges, triangles, influences and collapse tables of the older format
    }
    reader.i32()?;
    skip_array(reader, 4)?;
    reader.i32()?;
    reader.i32()?;
    if !reader.remaining().is_empty() {
        return Err(Error::TrailingBytes(reader.remaining().len()));
    }

    if !bones.iter().enumerate().all(|(index, bone)| bone.parent <= index) {
        return Err(Error::BadMesh);
    }
    let material = |&(slot, _): &(u16, Section)| {
        let texture = slots.get(usize::from(slot)).and_then(|&texture| usize::try_from(texture).ok());
        texture.and_then(|texture| textures.get(texture)).copied().unwrap_or(ObjectRef::Null)
    };
    let materials = lod.sections.iter().map(material).collect();
    let sections = lod.sections.iter().map(|&(_, section)| section).collect();
    Ok(SkeletalMesh {
        bones,
        vertices: lod.vertices,
        indices: lod.indices,
        sections,
        materials,
        animation,
        scale,
        origin,
        rotation,
    })
}

fn bone(package: &Package, reader: &mut Reader<'_>) -> Result<Bone, Error> {
    let name = package.name_at(reader.compact()?)?.to_owned();
    reader.u32()?; // flags
    let rotation = [reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?];
    let position = vector(reader)?;
    reader.bytes(16)?; // length and size
    reader.i32()?; // children
    let parent = usize::try_from(reader.i32()?).map_err(|_| Error::BadMesh)?;
    Ok(Bone { name, parent, rotation, position })
}

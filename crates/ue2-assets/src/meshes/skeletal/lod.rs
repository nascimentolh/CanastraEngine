//! A skeletal mesh's level of detail, turned into one skinned vertex list.
//!
//! Most parts keep their vertices in two streams: soft wedges weighted to up to four bones through their
//! section's bone map, and rigid vertices bound whole to their section's bone. Some keep the older layout
//! instead: shared points, wedges that give each point a UV, faces, and a list of bone influences per point.

use ue2_core::Reader;

use super::SkinVertex;
use crate::Error;
use crate::meshes::{Section, array, skip_array, vector};

pub(super) struct Lod {
    pub(super) vertices: Vec<SkinVertex>,
    pub(super) indices: Vec<u16>,
    /// Each section with its index into the mesh's material slots.
    pub(super) sections: Vec<(u16, Section)>,
}

struct RawSection {
    material: u16,
    bone: u16,
    first_face: u16,
    faces: u16,
    bone_map: Vec<i32>,
}

type SoftWedge = ([f32; 3], [f32; 3], [f32; 2], [u8; 4], [f32; 4]);
type RigidVertex = ([f32; 3], [f32; 3], [f32; 2]);

pub(super) fn read(reader: &mut Reader<'_>, bones: usize) -> Result<Lod, Error> {
    skip_array(reader, 4)?; // skinning data
    skip_array(reader, 16)?; // skin points
    reader.i32()?; // soft wedge count
    let soft_sections = array(reader, raw_section)?;
    let rigid_sections = array(reader, raw_section)?;
    let soft_indices = index_buffer(reader)?;
    let rigid_indices = index_buffer(reader)?;
    reader.bytes(12)?; // stream revision and two counters
    let rigid = array(reader, |reader| Ok((vector(reader)?, vector(reader)?, uv(reader)?)))?;
    let influences = lazy(reader, |reader| Ok((reader.f32()?, reader.u16()?, reader.u16()?)))?;
    let wedges = lazy(reader, |reader| Ok((reader.u16()?, uv(reader)?)))?;
    let faces = lazy(reader, |reader| Ok(([reader.u16()?, reader.u16()?, reader.u16()?], reader.u16()?)))?;
    let points = lazy(reader, vector)?;
    reader.bytes(4 * 6)?; // distance factor, hysteresis, shared vertices, max influences and two counters
    reader.i32()?; // uses new wedges
    let soft = array(reader, |reader| {
        let (position, normal, uv) = (vector(reader)?, vector(reader)?, uv(reader)?);
        let slots: [u8; 4] = reader.array()?;
        Ok((position, normal, uv, slots, [reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?]))
    })?;

    let lod = if soft.is_empty() && rigid.is_empty() {
        from_wedges(&points, &wedges, &faces, &influences)?
    } else {
        let mut lod = Lod { vertices: Vec::new(), indices: Vec::new(), sections: Vec::new() };
        soft_stream(&mut lod, &soft_sections, &soft_indices, soft)?;
        rigid_stream(&mut lod, &rigid_sections, &rigid_indices, rigid)?;
        lod
    };
    let bones_fit = lod.vertices.iter().all(|vertex| vertex.bones.iter().all(|&bone| usize::from(bone) < bones));
    let indices_fit = lod.indices.iter().all(|&index| usize::from(index) < lod.vertices.len());
    if bones_fit && indices_fit { Ok(lod) } else { Err(Error::BadMesh) }
}

fn soft_stream(lod: &mut Lod, sections: &[RawSection], indices: &[u16], soft: Vec<SoftWedge>) -> Result<(), Error> {
    // A wedge takes its bones from the section whose faces use it.
    let mut maps = vec![None; soft.len()];
    for section in sections {
        let faces = faces_of(indices, section)?;
        for &index in faces {
            *maps.get_mut(usize::from(index)).ok_or(Error::BadMesh)? = Some(&section.bone_map);
        }
        push_section(lod, section.material, faces, 0)?;
    }
    for ((position, normal, uv, slots, weights), map) in soft.into_iter().zip(maps) {
        let mut vertex = SkinVertex { position, normal, uv, bones: [0; 4], weights: [0.0; 4] };
        let targets = vertex.bones.iter_mut().zip(vertex.weights.iter_mut());
        for ((&local, &weight), (bone_slot, weight_slot)) in slots.iter().zip(&weights).zip(targets) {
            if local == 255 || weight <= 0.0 {
                continue;
            }
            let bone = map.and_then(|map| map.get(usize::from(local))).and_then(|&bone| u16::try_from(bone).ok());
            (*bone_slot, *weight_slot) = (bone.ok_or(Error::BadMesh)?, weight);
        }
        lod.vertices.push(vertex);
    }
    Ok(())
}

fn rigid_stream(lod: &mut Lod, sections: &[RawSection], indices: &[u16], rigid: Vec<RigidVertex>) -> Result<(), Error> {
    let offset = u16::try_from(lod.vertices.len()).map_err(|_| Error::BadMesh)?;
    let mut bones = vec![0; rigid.len()];
    for section in sections {
        let faces = faces_of(indices, section)?;
        for &index in faces {
            *bones.get_mut(usize::from(index)).ok_or(Error::BadMesh)? = section.bone;
        }
        push_section(lod, section.material, faces, offset)?;
    }
    for ((position, normal, uv), bone) in rigid.into_iter().zip(bones) {
        let (bones, weights) = ([bone, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]);
        lod.vertices.push(SkinVertex { position, normal, uv, bones, weights });
    }
    Ok(())
}

/// The older layout: a vertex per wedge, weighted by its point's four heaviest influences, with sections
/// made of consecutive faces that share a material. It carries no normals.
fn from_wedges(
    points: &[[f32; 3]],
    wedges: &[(u16, [f32; 2])],
    faces: &[([u16; 3], u16)],
    influences: &[(f32, u16, u16)],
) -> Result<Lod, Error> {
    let mut weights = vec![[(0_u16, 0.0_f32); 4]; points.len()];
    for &(weight, point, bone) in influences {
        let slots = weights.get_mut(usize::from(point)).ok_or(Error::BadMesh)?;
        let lightest = slots.iter_mut().min_by(|a, b| a.1.total_cmp(&b.1)).ok_or(Error::BadMesh)?;
        if weight > lightest.1 {
            *lightest = (bone, weight);
        }
    }
    let mut lod = Lod { vertices: Vec::with_capacity(wedges.len()), indices: Vec::new(), sections: Vec::new() };
    for &(point, uv) in wedges {
        let point = usize::from(point);
        let (&position, &slots) = points.get(point).zip(weights.get(point)).ok_or(Error::BadMesh)?;
        let (bones, weights) = (slots.map(|slot| slot.0), slots.map(|slot| slot.1));
        lod.vertices.push(SkinVertex { position, normal: [0.0; 3], uv, bones, weights });
    }
    for run in faces.chunk_by(|a, b| a.1 == b.1) {
        let indices: Vec<u16> = run.iter().flat_map(|face| face.0).collect();
        push_section(&mut lod, run.first().map_or(0, |face| face.1), &indices, 0)?;
    }
    Ok(lod)
}

fn push_section(lod: &mut Lod, material: u16, indices: &[u16], offset: u16) -> Result<(), Error> {
    let first_index = u32::try_from(lod.indices.len()).map_err(|_| Error::BadMesh)?;
    let triangles = u32::try_from(indices.len() / 3).map_err(|_| Error::BadMesh)?;
    for &index in indices {
        lod.indices.push(index.checked_add(offset).ok_or(Error::BadMesh)?);
    }
    lod.sections.push((material, Section { first_index, triangles }));
    Ok(())
}

fn raw_section(reader: &mut Reader<'_>) -> Result<RawSection, Error> {
    let material = reader.u16()?;
    reader.bytes(8)?; // stream and wedge index ranges
    let bone = reader.u16()?;
    reader.u16()?;
    let first_face = reader.u16()?;
    let faces = reader.u16()?;
    let bone_map = array(reader, |reader| Ok(reader.i32()?))?;
    Ok(RawSection { material, bone, first_face, faces, bone_map })
}

/// The indices of `section`'s faces.
fn faces_of<'a>(indices: &'a [u16], section: &RawSection) -> Result<&'a [u16], Error> {
    let first = usize::from(section.first_face) * 3;
    indices.get(first..first + usize::from(section.faces) * 3).ok_or(Error::BadMesh)
}

fn index_buffer(reader: &mut Reader<'_>) -> Result<Vec<u16>, Error> {
    let indices = array(reader, |reader| Ok(reader.u16()?))?;
    reader.u32()?; // revision
    Ok(indices)
}

/// An array stored after the offset that lets a loader skip it.
fn lazy<'a, T>(
    reader: &mut Reader<'a>,
    item: impl FnMut(&mut Reader<'a>) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    reader.u32()?;
    array(reader, item)
}

fn uv(reader: &mut Reader<'_>) -> Result<[f32; 2], Error> {
    Ok([reader.f32()?, reader.f32()?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(clippy::float_cmp, reason = "weights are copied, not computed")]
    fn older_layout_keeps_the_four_heaviest_influences_and_splits_sections_by_material() {
        let points = [[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let wedges = [(0, [0.0, 0.0]), (1, [1.0, 0.0]), (2, [0.0, 1.0])];
        let faces = [([0, 1, 2], 0), ([2, 1, 0], 0), ([0, 2, 1], 1)];
        let influences = [(0.1, 0, 1), (0.4, 0, 2), (0.2, 0, 3), (0.3, 0, 4), (0.5, 0, 5), (1.0, 1, 1), (1.0, 2, 2)];

        let lod = from_wedges(&points, &wedges, &faces, &influences).unwrap();

        let mut first: Vec<(u16, f32)> = lod.vertices[0].bones.into_iter().zip(lod.vertices[0].weights).collect();
        first.sort_by_key(|&(bone, _)| bone);
        assert_eq!(first, [(2, 0.4), (3, 0.2), (4, 0.3), (5, 0.5)]);
        assert_eq!(lod.vertices[1].uv, [1.0, 0.0]);
        let sections: Vec<(u16, u32, u32)> = lod
            .sections
            .iter()
            .map(|&(material, section)| (material, section.first_index, section.triangles))
            .collect();
        assert_eq!(sections, [(0, 0, 2), (1, 6, 1)]);
    }
}

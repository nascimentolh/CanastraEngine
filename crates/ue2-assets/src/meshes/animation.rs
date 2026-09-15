//! `MeshAnimation` objects as Lineage II High Five stores them: named sequences of per-bone keys.

use ue2_core::Reader;
use ue2_package::{Export, Package};

use super::{array, count, vector};
use crate::Error;
use crate::properties::object_properties_reader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimBone {
    pub name: String,
    /// Index of the parent bone; the root is its own parent.
    pub parent: usize,
}

/// One bone's keys. A track with a single rotation or position holds it for the whole sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// Index into `MeshAnimation::bones`.
    pub bone: usize,
    /// Quaternions (x, y, z, w).
    pub rotations: Vec<[f32; 4]>,
    pub positions: Vec<[f32; 3]>,
    /// Key times in frames.
    pub times: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    pub name: String,
    pub frames: u32,
    /// Frames per second.
    pub rate: f32,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshAnimation {
    pub bones: Vec<AnimBone>,
    pub sequences: Vec<Sequence>,
}

pub fn read_mesh_animation(package: &Package, file: &[u8], export: &Export) -> Result<MeshAnimation, Error> {
    super::skeletal::check_layout(package)?;
    let (_, mut reader) = object_properties_reader(package, file, export)?;
    let reader = &mut reader;
    reader.i32()?; // version
    let bones = array(reader, |reader| {
        let name = package.name_at(reader.compact()?)?.to_owned();
        reader.u32()?; // flags
        let parent = usize::try_from(reader.i32()?).map_err(|_| Error::BadMesh)?;
        Ok(AnimBone { name, parent })
    })?;

    // Motions are stored with the file offsets where the list and each motion end.
    let end = offset(reader)?;
    let motions = count(reader)?;
    if motions > reader.remaining().len() {
        return Err(Error::BadMesh);
    }
    let mut tracks = Vec::with_capacity(motions);
    for _ in 0..motions {
        let motion_end = offset(reader)?;
        tracks.push(motion(reader, bones.len())?);
        if reader.pos() != motion_end {
            return Err(Error::BadMesh);
        }
    }
    if reader.pos() != end {
        return Err(Error::BadMesh);
    }

    let sequences = array(reader, |reader| sequence(package, reader))?;
    if !reader.remaining().is_empty() {
        return Err(Error::TrailingBytes(reader.remaining().len()));
    }
    if sequences.len() != tracks.len() {
        return Err(Error::BadMesh);
    }
    let sequences = sequences
        .into_iter()
        .zip(tracks)
        .map(|((name, frames, rate), tracks)| Sequence { name, frames, rate, tracks })
        .collect();
    Ok(MeshAnimation { bones, sequences })
}

fn offset(reader: &mut Reader<'_>) -> Result<usize, Error> {
    usize::try_from(reader.i32()?).map_err(|_| Error::BadMesh)
}

fn motion(reader: &mut Reader<'_>, bones: usize) -> Result<Vec<Track>, Error> {
    reader.bytes(12 + 4 + 4 + 4)?; // root speed, track time, start bone and flags
    let mut bone_indices = array(reader, |reader| usize::try_from(reader.i32()?).map_err(|_| Error::BadMesh))?;
    let keys = array(reader, analog_track)?;
    analog_track(reader)?; // root track
    // Without bone indices, tracks follow the bones in order.
    if bone_indices.is_empty() {
        bone_indices = (0..keys.len()).collect();
    }
    if bone_indices.len() != keys.len() || bone_indices.iter().any(|&bone| bone >= bones) {
        return Err(Error::BadMesh);
    }
    Ok(bone_indices
        .into_iter()
        .zip(keys)
        .map(|(bone, (rotations, positions, times))| Track { bone, rotations, positions, times })
        .collect())
}

type Keys = (Vec<[f32; 4]>, Vec<[f32; 3]>, Vec<f32>);

fn analog_track(reader: &mut Reader<'_>) -> Result<Keys, Error> {
    reader.u32()?; // flags
    let rotations = array(reader, |reader| Ok([reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?]))?;
    let positions = array(reader, vector)?;
    let times = array(reader, |reader| Ok(reader.f32()?))?;
    Ok((rotations, positions, times))
}

fn sequence(package: &Package, reader: &mut Reader<'_>) -> Result<(String, u32, f32), Error> {
    reader.i32()?;
    let name = package.name_at(reader.compact()?)?.to_owned();
    array(reader, |reader| Ok(reader.compact()?))?; // groups
    reader.i32()?; // start frame
    let frames = u32::try_from(reader.i32()?).map_err(|_| Error::BadMesh)?;
    array(reader, |reader| {
        reader.f32()?; // time
        reader.compact()?; // function
        reader.compact()?; // notify object
        Ok(())
    })?; // notifications
    let rate = reader.f32()?;
    reader.bytes(4 * 3)?;
    reader.compact()?;
    reader.bytes(4 * 2)?;
    // A block of Lineage II extras: a flag, then pairs, groups of pairs, two numbers and pairs again.
    reader.u8()?;
    pairs(reader)?;
    array(reader, |reader| {
        reader.i32()?;
        pairs(reader)
    })?;
    reader.bytes(4 * 2)?;
    pairs(reader)?;
    Ok((name, frames, rate))
}

fn pairs(reader: &mut Reader<'_>) -> Result<(), Error> {
    super::skip_array(reader, 8)
}

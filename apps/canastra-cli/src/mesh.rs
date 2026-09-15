//! `canastra mesh`: reads a package's skeletal meshes and animations, or prints one of them.

use std::path::Path;

use ue2_package::{Export, ObjectRef, Package};

use crate::{Result, read_decrypted};

/// How many failures to print.
const EXAMPLES: usize = 5;

pub(crate) fn run(input: &Path, object: Option<&str>) -> Result {
    let (_, plain) = read_decrypted(input)?;
    let package = Package::parse(&plain)?;
    let is = |export: &Export, class: &str| package.class_name(export).eq_ignore_ascii_case(class);
    let path = |index: usize| package.object_path(ObjectRef::Export(index));
    let Some(object) = object else {
        let (mut meshes, mut animations, mut failures) = (0, 0, Vec::new());
        for (index, export) in package.exports().iter().enumerate() {
            let outcome = if is(export, "SkeletalMesh") {
                ue2_assets::read_skeletal_mesh(&package, &plain, export).map(|_| &mut meshes)
            } else if is(export, "MeshAnimation") {
                ue2_assets::read_mesh_animation(&package, &plain, export).map(|_| &mut animations)
            } else {
                continue;
            };
            match outcome {
                Ok(read) => *read += 1,
                Err(error) => failures.push(format!("{}: {error}", path(index))),
            }
        }
        println!("{meshes} skeletal meshes and {animations} animations read, {} failed", failures.len());
        for failure in failures.iter().take(EXAMPLES) {
            println!("  {failure}");
        }
        return if failures.is_empty() { Ok(()) } else { Err(format!("{} failure(s)", failures.len()).into()) };
    };
    let (_, export) = package
        .exports()
        .iter()
        .enumerate()
        .find(|&(index, _)| path(index).eq_ignore_ascii_case(object))
        .ok_or_else(|| format!("no object `{object}`"))?;
    if is(export, "MeshAnimation") {
        let animation = ue2_assets::read_mesh_animation(&package, &plain, export)?;
        println!("{} bones, {} sequences", animation.bones.len(), animation.sequences.len());
        for sequence in &animation.sequences {
            let keys: usize = sequence.tracks.iter().map(|track| track.rotations.len()).sum();
            println!(
                "  {:<32} {:>4} frames at {:>5} fps, {} tracks, {keys} rotation keys",
                sequence.name,
                sequence.frames,
                sequence.rate,
                sequence.tracks.len()
            );
        }
        return Ok(());
    }
    let mesh = ue2_assets::read_skeletal_mesh(&package, &plain, export)?;
    println!(
        "{} vertices, {} triangles, {} sections, {} bones, animation {}",
        mesh.vertices.len(),
        mesh.indices.len() / 3,
        mesh.sections.len(),
        mesh.bones.len(),
        package.object_path(mesh.animation)
    );
    println!("scale {:?} origin {:?} rotation {:?}", mesh.scale, mesh.origin, mesh.rotation);
    for (section, material) in mesh.sections.iter().zip(&mesh.materials) {
        println!("  section {section:?} material {}", package.object_path(*material));
    }
    for (index, bone) in mesh.bones.iter().enumerate() {
        println!(
            "  bone {index:>3} {:<24} parent {:>3} at {:?} turned {:?}",
            bone.name, bone.parent, bone.position, bone.rotation
        );
    }
    Ok(())
}

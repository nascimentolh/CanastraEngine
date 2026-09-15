//! Assets of an installed client found by path (`Package.Name` or `Package.Group.Name`) and read in
//! place: textures, static meshes and the texture a material draws with. Packages load on first use
//! and stay loaded.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ue2_assets::{Image, StaticMesh};
use ue2_package::{ObjectRef, Package};

/// Folders of the client that hold asset packages, with the extension used there.
const FOLDERS: [(&str, &str); 3] = [("SysTextures", "utx"), ("Textures", "utx"), ("StaticMeshes", "usx")];

/// Material properties that lead towards a base texture, most telling first.
const MATERIAL_INPUTS: [&str; 5] = ["Diffuse", "Material", "Material1", "Material2", "SelfIllumination"];

/// Deeper material chains than this are treated as cycles.
const MAX_MATERIAL_DEPTH: usize = 8;

#[derive(Default)]
pub struct Catalog {
    /// Lower-case package name to file.
    files: HashMap<String, PathBuf>,
    /// Lower-case package name to its contents, `None` when it failed to load.
    packages: HashMap<String, Option<Loaded>>,
}

struct Loaded {
    file: Vec<u8>,
    package: Package,
    /// Lower-case `Group.Name` and bare `Name` to export index.
    objects: HashMap<String, usize>,
}

/// A static mesh with each section's material as a client-wide path.
#[derive(Debug, Clone)]
pub struct Mesh {
    pub mesh: StaticMesh,
    /// Material of each section, `None` when unset.
    pub materials: Vec<Option<String>>,
}

impl Catalog {
    /// Indexes the asset packages under a client root; missing folders are skipped.
    pub fn open(client_root: &Path) -> Self {
        let mut files = HashMap::new();
        for (folder, extension) in FOLDERS {
            for entry in fs::read_dir(client_root.join(folder)).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
                    && let Some(stem) = path.file_stem()
                {
                    files.insert(stem.to_string_lossy().to_lowercase(), path);
                }
            }
        }
        Self { files, packages: HashMap::new() }
    }

    /// Decodes the texture at `path`, or `None` when it does not exist or cannot be read.
    pub fn texture(&mut self, path: &str) -> Option<Image> {
        let (loaded, index) = self.object(path, "Texture")?;
        ue2_assets::decode_texture(&loaded.package, &loaded.file, index).ok()
    }

    pub fn static_mesh(&mut self, path: &str) -> Option<Mesh> {
        let package_name = package_name(path)?;
        let (loaded, index) = self.object(path, "StaticMesh")?;
        let export = loaded.package.exports().get(index)?;
        let mesh = ue2_assets::read_static_mesh(&loaded.package, &loaded.file, export).ok()?;
        let materials =
            mesh.materials.iter().map(|&material| full_path(&package_name, &loaded.package, material)).collect();
        Some(Mesh { mesh, materials })
    }

    /// Path of the base texture `material` draws with, following shaders, combiners and modifiers.
    // ponytail: only the base texture is resolved; blending and animation come with material support.
    pub fn material_texture(&mut self, material: &str) -> Option<String> {
        let mut path = material.to_owned();
        for _ in 0..MAX_MATERIAL_DEPTH {
            let package_name = package_name(&path)?;
            let (loaded, index) = self.object(&path, "")?;
            let package = &loaded.package;
            let export = package.exports().get(index)?;
            if package.class_name(export).eq_ignore_ascii_case("Texture") {
                return Some(path);
            }
            let properties = ue2_assets::object_properties(package, &loaded.file, export).ok()?;
            path = MATERIAL_INPUTS.iter().find_map(|name| {
                let object = ue2_assets::find(&properties, name)?.object(package)?;
                full_path(&package_name, package, object)
            })?;
        }
        None
    }

    /// The loaded package and export index of `path`, when its class is `class` (any when empty).
    fn object(&mut self, path: &str, class: &str) -> Option<(&Loaded, usize)> {
        let path = path.to_lowercase();
        let (package_name, object) = path.split_once('.')?;
        let loaded = self.package(package_name)?;
        let index = *loaded.objects.get(object)?;
        let export = loaded.package.exports().get(index)?;
        (class.is_empty() || loaded.package.class_name(export).eq_ignore_ascii_case(class)).then_some((loaded, index))
    }

    fn package(&mut self, name: &str) -> Option<&Loaded> {
        if !self.packages.contains_key(name) {
            let loaded = self.files.get(name).and_then(|path| load(path));
            self.packages.insert(name.to_owned(), loaded);
        }
        self.packages.get(name)?.as_ref()
    }
}

fn package_name(path: &str) -> Option<String> {
    path.split_once('.').map(|(package, _)| package.to_lowercase())
}

/// A reference read inside `package` as a client-wide path: imports already start with their
/// package, exports get the package's own name in front.
fn full_path(package_name: &str, package: &Package, object: ObjectRef) -> Option<String> {
    match object {
        ObjectRef::Null => None,
        ObjectRef::Import(_) => Some(package.object_path(object)),
        ObjectRef::Export(_) => Some(format!("{package_name}.{}", package.object_path(object))),
    }
}

fn load(path: &Path) -> Option<Loaded> {
    let bytes = fs::read(path).ok()?;
    let file = l2_crypto::decrypt(&bytes, path).ok()?.into_owned();
    let package = Package::parse(&file).ok()?;
    let mut objects = HashMap::new();
    for (index, export) in package.exports().iter().enumerate() {
        // Two-part references name the object without its group, so index both forms.
        objects.insert(package.object_path(ObjectRef::Export(index)).to_lowercase(), index);
        objects.entry(package.name(export.name).to_lowercase()).or_insert(index);
    }
    Some(Loaded { file, package, objects })
}

//! Textures of an installed client, found by path (`Package.Name` or `Package.Group.Name`) and
//! decoded in place. Packages load on first use and stay loaded.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ue2_assets::Image;
use ue2_package::{ObjectRef, Package};

/// Folders of the client that hold texture packages.
const TEXTURE_FOLDERS: [&str; 2] = ["SysTextures", "Textures"];

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
    textures: HashMap<String, usize>,
}

impl Catalog {
    /// Indexes the texture packages under a client root; missing folders are skipped.
    pub fn open(client_root: &Path) -> Self {
        let mut files = HashMap::new();
        for folder in TEXTURE_FOLDERS.map(|folder| client_root.join(folder)) {
            for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("utx"))
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
        let path = path.to_lowercase();
        let (package_name, object) = path.split_once('.')?;
        let loaded = self.package(package_name)?;
        let index = *loaded.textures.get(object)?;
        ue2_assets::decode_texture(&loaded.package, &loaded.file, index).ok()
    }

    fn package(&mut self, name: &str) -> Option<&Loaded> {
        if !self.packages.contains_key(name) {
            let loaded = self.files.get(name).and_then(|path| load(path));
            self.packages.insert(name.to_owned(), loaded);
        }
        self.packages.get(name)?.as_ref()
    }
}

fn load(path: &Path) -> Option<Loaded> {
    let bytes = fs::read(path).ok()?;
    let file = l2_crypto::decrypt(&bytes, path).ok()?.into_owned();
    let package = Package::parse(&file).ok()?;
    let mut textures = HashMap::new();
    for (index, export) in package.exports().iter().enumerate() {
        if package.class_name(export).eq_ignore_ascii_case("Texture") {
            // Two-part references name the object without its group, so index both forms.
            textures.insert(package.object_path(ObjectRef::Export(index)).to_lowercase(), index);
            textures.entry(package.name(export.name).to_lowercase()).or_insert(index);
        }
    }
    Some(Loaded { file, package, textures })
}

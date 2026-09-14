//! Icons read in place from the client's texture packages and cached as egui textures.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use canastra_data::asset::TextureRef;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions, Ui, Vec2};
use ue2_package::{ObjectRef, Package};

/// Folders of the client that hold texture packages.
const TEXTURE_FOLDERS: [&str; 2] = ["SysTextures", "Textures"];

pub(crate) struct Icons {
    /// Lower-case package name to file.
    files: HashMap<String, PathBuf>,
    /// Lower-case package name to its loaded contents, `None` when it failed to load.
    packages: HashMap<String, Option<Loaded>>,
    /// Lower-case asset path to its texture, `None` when it failed to resolve.
    textures: HashMap<String, Option<TextureHandle>>,
}

struct Loaded {
    file: Vec<u8>,
    package: Package,
    /// Lower-case `Group.Name` and bare `Name` to export index.
    exports: HashMap<String, usize>,
}

impl Icons {
    pub(crate) fn new(client_root: Option<&Path>) -> Self {
        let mut files = HashMap::new();
        for folder in client_root.into_iter().flat_map(|root| TEXTURE_FOLDERS.map(|folder| root.join(folder))) {
            for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("utx"))
                    && let Some(stem) = path.file_stem()
                {
                    files.insert(stem.to_string_lossy().to_lowercase(), path);
                }
            }
        }
        Self { files, packages: HashMap::new(), textures: HashMap::new() }
    }

    /// Draws the icon, or an empty square of the same size when it cannot be resolved.
    pub(crate) fn show(&mut self, ui: &mut Ui, icon: Option<&TextureRef>, size: f32) {
        let texture = icon.and_then(|icon| self.texture(ui.ctx(), icon.path()));
        match texture {
            Some(texture) => {
                ui.image(egui::load::SizedTexture::new(texture.id(), Vec2::splat(size)));
            }
            None => {
                ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
            }
        }
    }

    fn texture(&mut self, ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
        let key = path.to_lowercase();
        if let Some(cached) = self.textures.get(&key) {
            return cached.clone();
        }
        let texture = self.decode(&key).map(|image| {
            let size = [image.width as usize, image.height as usize];
            ctx.load_texture(&key, ColorImage::from_rgba_unmultiplied(size, &image.rgba), TextureOptions::LINEAR)
        });
        self.textures.insert(key, texture.clone());
        texture
    }

    /// `path` is lower-case `package.name` or `package.group.name`.
    fn decode(&mut self, path: &str) -> Option<ue2_assets::Image> {
        let (package_name, object) = path.split_once('.')?;
        let loaded = self.package(package_name)?;
        let index = *loaded.exports.get(object)?;
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
    let mut exports = HashMap::new();
    for (index, export) in package.exports().iter().enumerate() {
        if package.class_name(export).eq_ignore_ascii_case("Texture") {
            // Two-part references name the object without its group, so index both forms.
            exports.insert(package.object_path(ObjectRef::Export(index)).to_lowercase(), index);
            exports.entry(package.name(export.name).to_lowercase()).or_insert(index);
        }
    }
    Some(Loaded { file, package, exports })
}

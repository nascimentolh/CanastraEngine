//! Client textures as egui textures, decoded once per path.

use std::collections::HashMap;

use canastra_data::asset::TextureRef;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions, Ui, Vec2};
use l2_catalog::Catalog;

pub(crate) struct Icons {
    catalog: Catalog,
    /// Lower-case asset path to its texture, `None` when it failed to resolve.
    textures: HashMap<String, Option<TextureHandle>>,
}

impl Icons {
    pub(crate) fn new(catalog: Catalog) -> Self {
        Self { catalog, textures: HashMap::new() }
    }

    /// Draws the icon, or an empty square of the same size when it cannot be resolved.
    pub(crate) fn show(&mut self, ui: &mut Ui, icon: Option<&TextureRef>, size: f32) {
        match icon.and_then(|icon| self.texture(ui.ctx(), icon.path())) {
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
        let texture = self.catalog.texture(&key).map(|image| {
            let size = [image.width as usize, image.height as usize];
            ctx.load_texture(&key, ColorImage::from_rgba_unmultiplied(size, &image.rgba), TextureOptions::LINEAR)
        });
        self.textures.insert(key, texture.clone());
        texture
    }
}

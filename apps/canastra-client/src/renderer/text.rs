//! Text through glyphon: one font system measures for layout and shapes for drawing.

use std::path::Path;

use canastra_ui::{Rect, Rgba, TextAlign, TextMeasure, TextStyle};
use glyphon::cosmic_text::Align;
use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea, TextAtlas,
    TextBounds, TextRenderer, Viewport,
};

/// Line height as a multiple of the font size.
const LINE_HEIGHT: f32 = 1.25;

/// A text draw of the frame, in logical pixels.
pub(super) struct Label<'a> {
    pub(super) rect: Rect,
    pub(super) text: &'a str,
    pub(super) color: Rgba,
    pub(super) style: &'a TextStyle,
}

pub(crate) struct Fonts(FontSystem);

impl Fonts {
    /// Adds every `.ttf` and `.otf` in `folder` to the system fonts; returns how many faces loaded.
    pub(crate) fn load_folder(&mut self, folder: &Path) -> usize {
        let before = self.0.db().len();
        self.0.db_mut().load_fonts_dir(folder);
        self.0.db().len() - before
    }

    /// Shapes `text` in `style` scaled by `scale`, wrapping at `width` physical pixels.
    fn buffer(&mut self, text: &str, style: &TextStyle, scale: f32, width: Option<f32>) -> Buffer {
        let size = style.size * scale;
        let mut buffer = Buffer::new(&mut self.0, Metrics::new(size, size * LINE_HEIGHT));
        buffer.set_size(width, None);
        let attrs = Attrs::new()
            .family(style.family.as_deref().map_or(Family::SansSerif, Family::Name))
            .weight(glyphon::Weight(style.weight))
            .letter_spacing(style.letter_spacing / style.size.max(1.0));
        let align = match style.align {
            TextAlign::Left => None,
            TextAlign::Center => Some(Align::Center),
            TextAlign::Right => Some(Align::Right),
        };
        buffer.set_text(text, &attrs, Shaping::Advanced, align);
        buffer.shape_until_scroll(&mut self.0, false);
        buffer
    }
}

impl TextMeasure for Fonts {
    fn measure(&mut self, text: &str, style: &TextStyle, max_width: Option<f32>) -> (f32, f32) {
        let buffer = self.buffer(text, style, 1.0, max_width);
        let (width, lines) =
            buffer.layout_runs().fold((0.0_f32, 0_u16), |(width, lines), run| (width.max(run.line_w), lines + 1));
        (width.ceil(), f32::from(lines.max(1)) * style.size * LINE_HEIGHT)
    }
}

pub(super) struct Text {
    pub(super) fonts: Fonts,
    swash: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    /// One renderer for the text under the overlays and one for the overlays' own.
    renderers: [TextRenderer; 2],
}

impl Text {
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let renderers =
            [(), ()].map(|()| TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None));
        Self { fonts: Fonts(FontSystem::new()), swash: SwashCache::new(), viewport, atlas, renderers }
    }

    /// Shapes `labels` and uploads their glyphs for a target of `size` physical pixels, the text under the
    /// overlays and that of the overlays apart.
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        scale: f32,
        layers: [&[Label<'_>]; 2],
    ) -> Result<(), glyphon::PrepareError> {
        self.viewport.update(queue, Resolution { width: size[0], height: size[1] });
        for (renderer, labels) in self.renderers.iter_mut().zip(layers) {
            let buffers: Vec<Buffer> = labels
                .iter()
                .map(|label| {
                    let mut buffer = self.fonts.buffer(label.text, label.style, scale, Some(label.rect.width * scale));
                    buffer.set_size(Some(label.rect.width * scale), Some(label.rect.height * scale));
                    buffer
                })
                .collect();
            let areas = labels.iter().zip(&buffers).map(|(label, buffer)| {
                let Rgba([r, g, b, a]) = label.color;
                let Rect { x, y, width, height } = label.rect;
                TextArea {
                    buffer,
                    left: x * scale,
                    top: y * scale,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: (x * scale) as i32,
                        top: (y * scale) as i32,
                        right: ((x + width) * scale).ceil() as i32,
                        bottom: ((y + height) * scale).ceil() as i32,
                    },
                    default_color: Color::rgba(r, g, b, a),
                    custom_glyphs: &[],
                }
            });
            renderer.prepare(
                device,
                queue,
                &mut self.fonts.0,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash,
            )?;
        }
        Ok(())
    }

    /// Draws the text under the overlays, or with `overlay` that of the overlays, which also ends the frame.
    pub(super) fn draw(&mut self, pass: &mut wgpu::RenderPass<'_>, overlay: bool) -> Result<(), glyphon::RenderError> {
        let [base, overlays] = &self.renderers;
        (if overlay { overlays } else { base }).render(&self.atlas, &self.viewport, pass)?;
        if overlay {
            self.atlas.trim();
        }
        Ok(())
    }
}

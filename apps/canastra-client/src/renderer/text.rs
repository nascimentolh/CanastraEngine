//! Text through glyphon: one font system measures for layout and shapes for drawing.

use canastra_ui::{Rect, Rgba, TextMeasure};
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
    pub(super) size: f32,
}

pub(crate) struct Fonts(FontSystem);

impl Fonts {
    fn buffer(&mut self, text: &str, size: f32, width: Option<f32>) -> Buffer {
        let mut buffer = Buffer::new(&mut self.0, Metrics::new(size, size * LINE_HEIGHT));
        buffer.set_size(width, None);
        buffer.set_text(text, &Attrs::new().family(Family::SansSerif), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.0, false);
        buffer
    }
}

impl TextMeasure for Fonts {
    fn measure(&mut self, text: &str, size: f32, max_width: Option<f32>) -> (f32, f32) {
        let buffer = self.buffer(text, size, max_width);
        let (width, lines) =
            buffer.layout_runs().fold((0.0_f32, 0_u16), |(width, lines), run| (width.max(run.line_w), lines + 1));
        (width.ceil(), f32::from(lines.max(1)) * size * LINE_HEIGHT)
    }
}

pub(super) struct Text {
    pub(super) fonts: Fonts,
    swash: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    renderer: TextRenderer,
}

impl Text {
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let renderer = TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        Self { fonts: Fonts(FontSystem::new()), swash: SwashCache::new(), viewport, atlas, renderer }
    }

    /// Shapes `labels` and uploads their glyphs for a target of `size` physical pixels.
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        scale: f32,
        labels: &[Label<'_>],
    ) -> Result<(), glyphon::PrepareError> {
        let buffers: Vec<Buffer> = labels
            .iter()
            .map(|label| {
                let mut buffer = self.fonts.buffer(label.text, label.size * scale, Some(label.rect.width * scale));
                buffer.set_size(Some(label.rect.width * scale), Some(label.rect.height * scale));
                buffer
            })
            .collect();
        self.viewport.update(queue, Resolution { width: size[0], height: size[1] });
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
        self.renderer.prepare(device, queue, &mut self.fonts.0, &mut self.atlas, &self.viewport, areas, &mut self.swash)
    }

    pub(super) fn draw(&mut self, pass: &mut wgpu::RenderPass<'_>) -> Result<(), glyphon::RenderError> {
        self.renderer.render(&self.atlas, &self.viewport, pass)?;
        self.atlas.trim();
        Ok(())
    }
}

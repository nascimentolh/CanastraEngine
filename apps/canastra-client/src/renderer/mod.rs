//! Draws a `canastra_ui::Frame` with wgpu: instanced quads for shapes and images, glyphon for text.

mod quad;
mod shapes;
mod text;

use canastra_ui::{Draw, Frame};

use crate::gpu::Gpu;
use l2_catalog::Catalog;

pub(crate) use text::Fonts;

pub(crate) struct Renderer {
    shapes: shapes::Shapes,
    text: text::Text,
}

impl Renderer {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        catalog: Catalog,
    ) -> Self {
        Self {
            shapes: shapes::Shapes::new(device, queue, format, catalog),
            text: text::Text::new(device, queue, format),
        }
    }

    /// The fonts text is drawn with, for measuring during layout.
    pub(crate) fn fonts(&mut self) -> &mut Fonts {
        &mut self.text.fonts
    }

    /// Draws `frame` (in logical pixels) onto the window's `target`, over what it already shows or,
    /// with `clear`, over black.
    pub(crate) fn render(
        &mut self,
        gpu: &Gpu,
        target: &wgpu::TextureView,
        frame: &Frame,
        clear: bool,
    ) -> Result<wgpu::CommandBuffer, String> {
        let (device, queue, size, scale) = (&gpu.device, &gpu.queue, gpu.size(), gpu.scale());
        // Two layers, each its shapes then its text: the frame, then its overlays over it.
        // ponytail: within a layer text draws over every shape; split more layers when windows overlap.
        self.shapes.prepare(device, queue, size, scale, &frame.draws, frame.overlay_from);
        let (base, overlays) = frame.draws.split_at(frame.overlay_from.min(frame.draws.len()));
        let (base, overlays) = (labels(base), labels(overlays));
        self.text.prepare(device, queue, size, scale, [&base, &overlays]).map_err(|error| error.to_string())?;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("ui") });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear { wgpu::LoadOp::Clear(wgpu::Color::BLACK) } else { wgpu::LoadOp::Load },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        for overlay in [false, true] {
            self.shapes.draw(&mut pass, overlay);
            self.text.draw(&mut pass, overlay).map_err(|error| error.to_string())?;
        }
        drop(pass);
        Ok(encoder.finish())
    }
}

/// The text draws of `draws`.
fn labels(draws: &[Draw]) -> Vec<text::Label<'_>> {
    draws
        .iter()
        .filter_map(|draw| match draw {
            Draw::Text { rect, text, color, style } => Some(text::Label { rect: *rect, text, color: *color, style }),
            _ => None,
        })
        .collect()
}

//! Draws a `canastra_ui::Frame` with wgpu: instanced quads for shapes and images, glyphon for text.

mod quad;
mod shapes;
mod text;

use canastra_ui::{Draw, Frame};
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

    /// Draws `frame` (in logical pixels) over a cleared target of `size` physical pixels.
    pub(crate) fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: [u32; 2],
        scale: f32,
        frame: &Frame,
    ) -> Result<wgpu::CommandBuffer, String> {
        // ponytail: text always draws over every shape; interleave passes when windows overlap.
        self.shapes.prepare(device, queue, size, scale, &frame.draws);
        let labels: Vec<text::Label<'_>> = frame
            .draws
            .iter()
            .filter_map(|draw| match draw {
                Draw::Text { rect, text, color, style } => {
                    Some(text::Label { rect: *rect, text, color: *color, style })
                }
                _ => None,
            })
            .collect();
        self.text.prepare(device, queue, size, scale, &labels).map_err(|error| error.to_string())?;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("ui") });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.shapes.draw(&mut pass);
        self.text.draw(&mut pass).map_err(|error| error.to_string())?;
        drop(pass);
        Ok(encoder.finish())
    }
}

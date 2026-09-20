//! Standing characters in a loaded scene.

use std::collections::HashMap;

use super::load::Vertex;
use super::pawns::{self, Figure, Pawn};
use super::pipeline::{Draw, Pipeline, draw_order};
use super::{Scene, buffer, index_bytes, material_batch};
use crate::gpu::Gpu;

impl Scene {
    /// Stands `figures` in the scene in place of the characters there.
    pub(crate) fn place(&mut self, gpu: &Gpu, figures: &[Figure]) {
        let (device, queue) = (&gpu.device, &gpu.queue);
        // A character of the world stands on the floor the map puts under it, not at the height it was stored at.
        let grounded: Vec<Figure> = figures
            .iter()
            .map(|figure| {
                if figure.grounded {
                    Figure { location: self.on_ground(figure.location), ..figure.clone() }
                } else {
                    figure.clone()
                }
            })
            .collect();
        self.pawns = grounded.iter().map(|figure| Pawn::load(&mut self.catalog, figure)).collect();
        let layout = pawns::layout(&self.pawns);
        let mut views: HashMap<String, wgpu::TextureView> = HashMap::new();
        let stages = layout.ranges.iter().flat_map(|(material, _)| {
            std::iter::once(&material.base).chain(material.layer.as_ref().map(|(stage, _, _)| stage))
        });
        for stage in stages {
            if !views.contains_key(&stage.texture)
                && let Some(pixels) = self.catalog.pixels(&stage.texture, gpu.blocks)
            {
                views.insert(stage.texture.clone(), Pipeline::texture(device, queue, &pixels));
            }
        }
        let view = |path: &str| views.get(path);
        self.pawn_batches = layout
            .ranges
            .iter()
            .filter_map(|(material, range)| {
                let draw = Draw::surface(material.blend);
                material_batch(&mut self.pipeline, device, &view, material.clone(), draw, range.clone())
            })
            .collect();
        // Hair blends over the face and collar under it; drawn first, it would keep them out of the depth it writes.
        self.pawn_batches.sort_by_key(|batch| draw_order(batch.material.blend));
        (self.pawn_vertices, self.pawn_indices) = pawn_buffers(device, &layout);
        self.uniforms_written = false;
    }
}

/// A vertex buffer the pawns rewrite every frame and their fixed index buffer; never empty.
pub(super) fn pawn_buffers(device: &wgpu::Device, layout: &pawns::Layout) -> (wgpu::Buffer, wgpu::Buffer) {
    let vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pawn vertices"),
        size: (layout.vertices.max(1) * size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let indices = if layout.indices.is_empty() { vec![0; 3] } else { layout.indices.clone() };
    (vertices, buffer(device, "pawn indices", wgpu::BufferUsages::INDEX, &index_bytes(&indices)))
}

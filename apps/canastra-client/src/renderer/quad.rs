//! CPU side of `ui.wgsl`: frame rectangles and images as shader quads in physical pixels.

use canastra_ui::{Fill, Rect, Rgba};

pub(super) const QUAD_BYTES: u64 = 96;

/// One quad as the shader reads it.
#[derive(Debug, PartialEq)]
pub(super) struct Quad {
    rect: [f32; 4],
    uv: [f32; 4],
    top: [f32; 4],
    bottom: [f32; 4],
    border: [f32; 4],
    params: [f32; 4],
}

impl Quad {
    pub(super) fn write(&self, bytes: &mut Vec<u8>) {
        for values in [self.rect, self.uv, self.top, self.bottom, self.border, self.params] {
            bytes.extend(values.iter().flat_map(|value| value.to_le_bytes()));
        }
    }

    fn textured(rect: [f32; 4], uv: [f32; 4]) -> Self {
        Self { rect, uv, top: [0.0; 4], bottom: [0.0; 4], border: [0.0; 4], params: [0.0, 0.0, 1.0, 0.0] }
    }
}

pub(super) fn shape(rect: Rect, fill: Option<Fill>, border: Option<(f32, Rgba)>, radius: f32, scale: f32) -> Quad {
    let (top, bottom) = match fill {
        Some(Fill::Solid(color)) => (linear(color), linear(color)),
        Some(Fill::Vertical(top, bottom)) => (linear(top), linear(bottom)),
        None => ([0.0; 4], [0.0; 4]),
    };
    let (width, color) = border.map_or((0.0, [0.0; 4]), |(width, color)| (width * scale, linear(color)));
    Quad {
        rect: physical(rect, scale),
        uv: [0.0, 0.0, 1.0, 1.0],
        top,
        bottom,
        border: color,
        params: [radius * scale, width, 0.0, 0.0],
    }
}

/// Splits an image into nine quads whose corners keep `inset` texture pixels unscaled;
/// with no inset it is one stretched quad.
pub(super) fn nine_slice(rect: Rect, texture: [f32; 2], inset: f32, scale: f32) -> Vec<Quad> {
    let [x, y, width, height] = physical(rect, scale);
    if inset <= 0.0 {
        return vec![Quad::textured([x, y, width, height], [0.0, 0.0, 1.0, 1.0])];
    }
    let edge_x = (inset * scale).min(width / 2.0);
    let edge_y = (inset * scale).min(height / 2.0);
    let xs = [x, x + edge_x, x + width - edge_x, x + width];
    let ys = [y, y + edge_y, y + height - edge_y, y + height];
    let us = [0.0, inset / texture[0], 1.0 - inset / texture[0], 1.0];
    let vs = [0.0, inset / texture[1], 1.0 - inset / texture[1], 1.0];
    let cuts = |values: [f32; 4]| values.into_iter().zip(values.into_iter().skip(1)).collect::<Vec<_>>();
    let (xs, ys, us, vs) = (cuts(xs), cuts(ys), cuts(us), cuts(vs));
    let rows = ys.iter().zip(&vs);
    rows.flat_map(|(&(y0, y1), &(v0, v1))| {
        xs.iter()
            .zip(&us)
            .map(move |(&(x0, x1), &(u0, u1))| Quad::textured([x0, y0, x1 - x0, y1 - y0], [u0, v0, u1, v1]))
    })
    .collect()
}

fn physical(rect: Rect, scale: f32) -> [f32; 4] {
    [rect.x * scale, rect.y * scale, rect.width * scale, rect.height * scale]
}

/// CSS colors are sRGB; the shader blends in linear space.
fn linear(Rgba([r, g, b, a]): Rgba) -> [f32; 4] {
    let channel = |value: u8| {
        let value = f32::from(value) / 255.0;
        if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
    };
    [channel(r), channel(g), channel(b), f32::from(a) / 255.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(clippy::float_cmp, reason = "every value is an exact binary fraction")]
    fn nine_slice_keeps_corners_and_stretches_the_middle() {
        let rect = Rect { x: 10.0, y: 20.0, width: 100.0, height: 40.0 };
        let quads = nine_slice(rect, [32.0, 16.0], 4.0, 2.0);
        assert_eq!(quads.len(), 9);
        assert_eq!(quads[0], Quad::textured([20.0, 40.0, 8.0, 8.0], [0.0, 0.0, 0.125, 0.25]));
        assert_eq!(quads[4], Quad::textured([28.0, 48.0, 184.0, 64.0], [0.125, 0.25, 0.875, 0.75]));
        assert_eq!(quads[8].rect, [212.0, 112.0, 8.0, 8.0]);
        let mut bytes = Vec::new();
        quads[0].write(&mut bytes);
        assert_eq!(bytes.len() as u64, QUAD_BYTES);
    }
}

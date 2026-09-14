//! CSS transitions: when an element's paint changes, it eases from what was on screen to the new style.

use std::collections::HashMap;

use crate::{Fill, Rgba, Shadow};

/// The properties a transition animates.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Paint {
    pub(crate) fill: Option<Fill>,
    pub(crate) border: Option<(f32, Rgba)>,
    pub(crate) shadows: Vec<Shadow>,
    pub(crate) color: Rgba,
}

/// Running transitions of one document, by pre-order element index.
#[derive(Debug, Default)]
pub struct Transitions {
    running: HashMap<usize, Animation>,
}

#[derive(Debug)]
struct Animation {
    from: Paint,
    to: Paint,
    /// Seconds, on the clock the caller passes as `UiState::time`.
    start: f32,
    duration: f32,
}

impl Transitions {
    /// What `element` shows at `now` when its style asks for `target`, and whether it is still moving.
    pub(crate) fn paint(&mut self, element: usize, target: &Paint, duration: f32, now: f32) -> (Paint, bool) {
        if duration <= 0.0 {
            self.running.remove(&element);
            return (target.clone(), false);
        }
        let animation = self.running.entry(element).or_insert_with(|| Animation {
            from: target.clone(),
            to: target.clone(),
            start: f32::NEG_INFINITY,
            duration,
        });
        if animation.to != *target {
            *animation = Animation { from: animation.at(now), to: target.clone(), start: now, duration };
        }
        (animation.at(now), animation.progress(now) < 1.0)
    }
}

impl Animation {
    fn progress(&self, now: f32) -> f32 {
        ((now - self.start) / self.duration).clamp(0.0, 1.0)
    }

    fn at(&self, now: f32) -> Paint {
        let progress = self.progress(now);
        if progress >= 1.0 {
            return self.to.clone();
        }
        // Ease out: fast start, gentle stop.
        let t = 1.0 - (1.0 - progress).powi(3);
        let (from, to) = (&self.from, &self.to);
        let shadows = (0..from.shadows.len().max(to.shadows.len()))
            .filter_map(|index| {
                // A layer only one side has fades in or out from a transparent copy of itself.
                let (a, b) = match (from.shadows.get(index), to.shadows.get(index)) {
                    (Some(a), Some(b)) => (*a, *b),
                    (Some(a), None) => (*a, Shadow { color: clear(a.color), ..*a }),
                    (None, Some(b)) => (Shadow { color: clear(b.color), ..*b }, *b),
                    (None, None) => return None,
                };
                Some(Shadow {
                    offset: [lerp(a.offset[0], b.offset[0], t), lerp(a.offset[1], b.offset[1], t)],
                    blur: lerp(a.blur, b.blur, t),
                    color: mix(a.color, b.color, t),
                    inset: b.inset,
                })
            })
            .collect();
        Paint {
            fill: match (from.fill, to.fill) {
                (Some(a), Some(b)) => Some(mix_fill(a, b, t)),
                (Some(a), None) => Some(mix_fill(a, map_fill(a, clear), t)),
                (None, Some(b)) => Some(mix_fill(map_fill(b, clear), b, t)),
                (None, None) => None,
            },
            border: match (from.border, to.border) {
                (Some((wa, ca)), Some((wb, cb))) => Some((lerp(wa, wb, t), mix(ca, cb, t))),
                (Some((width, color)), None) => Some((width, mix(color, clear(color), t))),
                (None, Some((width, color))) => Some((width, mix(clear(color), color, t))),
                (None, None) => None,
            },
            shadows,
            color: mix(from.color, to.color, t),
        }
    }
}

/// Fills of different kinds blend as vertical gradients; radial ones jump unless both are radial.
fn mix_fill(from: Fill, to: Fill, t: f32) -> Fill {
    match (from, to) {
        (Fill::Solid(a), Fill::Solid(b)) => Fill::Solid(mix(a, b, t)),
        (Fill::Radial(a1, a2), Fill::Radial(b1, b2)) => Fill::Radial(mix(a1, b1, t), mix(a2, b2, t)),
        (Fill::Radial(..), _) | (_, Fill::Radial(..)) => to,
        (from, to) => {
            let (a1, a2) = vertical(from);
            let (b1, b2) = vertical(to);
            Fill::Vertical(mix(a1, b1, t), mix(a2, b2, t))
        }
    }
}

fn vertical(fill: Fill) -> (Rgba, Rgba) {
    match fill {
        Fill::Solid(color) => (color, color),
        Fill::Vertical(top, bottom) | Fill::Radial(top, bottom) => (top, bottom),
    }
}

fn map_fill(fill: Fill, f: fn(Rgba) -> Rgba) -> Fill {
    match fill {
        Fill::Solid(color) => Fill::Solid(f(color)),
        Fill::Vertical(top, bottom) => Fill::Vertical(f(top), f(bottom)),
        Fill::Radial(center, corners) => Fill::Radial(f(center), f(corners)),
    }
}

fn clear(Rgba([r, g, b, _]): Rgba) -> Rgba {
    Rgba([r, g, b, 0])
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a blend of two u8 values stays in 0..=255"
)]
fn mix(Rgba(from): Rgba, Rgba(to): Rgba, t: f32) -> Rgba {
    let mut out = [0; 4];
    for ((out, from), to) in out.iter_mut().zip(from).zip(to) {
        *out = lerp(f32::from(from), f32::from(to), t).round() as u8;
    }
    Rgba(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint(color: [u8; 4], shadows: Vec<Shadow>) -> Paint {
        Paint { fill: Some(Fill::Solid(Rgba(color))), border: None, shadows, color: Rgba(color) }
    }

    #[test]
    fn eases_from_what_is_on_screen_to_the_new_style() {
        let mut transitions = Transitions::default();
        let (black, white) = (paint([0, 0, 0, 255], vec![]), paint([255; 4], vec![]));
        assert_eq!(transitions.paint(1, &black, 0.2, 0.0), (black.clone(), false));

        let glow = Shadow { offset: [0.0, 0.0], blur: 10.0, color: Rgba([255, 0, 0, 200]), inset: true };
        let hovered = Paint { shadows: vec![glow], ..white.clone() };
        let (start, moving) = transitions.paint(1, &hovered, 0.2, 1.0);
        assert!(moving && start.color == black.color);
        let (halfway, moving) = transitions.paint(1, &hovered, 0.2, 1.1);
        // Half the time with ease-out covers seven eighths of the way.
        assert!(moving);
        assert_eq!(halfway.color, Rgba([223, 223, 223, 255]));
        assert_eq!(halfway.shadows[0].color, Rgba([255, 0, 0, 175]));

        // Changing target mid-way starts from the value on screen, not from the old target.
        let (reversed, _) = transitions.paint(1, &black, 0.2, 1.1);
        assert_eq!(reversed.color, Rgba([223, 223, 223, 255]));
        assert_eq!(transitions.paint(1, &black, 0.2, 5.0), (black, false));
    }
}

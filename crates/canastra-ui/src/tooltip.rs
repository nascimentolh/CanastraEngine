//! The tip of the element under the pointer: a box styled by `.tooltip`, just below the element, or above it when the
//! viewport runs out below.

use crate::css::{States, StyleSheet};
use crate::layout::{ROOT, apply, default_style};
use crate::markup::{Element, Tag};
use crate::{Draw, Rect, TextMeasure};

/// How far the box stands from the element it explains, and from the viewport's edges, in pixels.
const GAP: f32 = 6.0;
/// The box's width when the stylesheet gives none.
const WIDTH: f32 = 260.0;

/// The draws of the box explaining the element at `anchor` with `tip`, kept inside `viewport`.
pub(crate) fn draws(
    sheet: &StyleSheet,
    viewport: [f32; 2],
    anchor: Rect,
    tip: &str,
    text: &mut dyn TextMeasure,
) -> Vec<Draw> {
    let mut element = Element::new(Tag::Label);
    element.classes.push("tooltip".into());
    let (mut style, mut computed) = (default_style(Tag::Label), ROOT);
    for declaration in sheet.cascade(&element, States::default()) {
        apply(declaration, &mut style, &mut computed);
    }
    let width = style.size.width.into_option().unwrap_or(WIDTH);
    let [top, right, bottom, left] = computed.padding;
    let (_, height) = text.measure(tip, &computed.text, Some(width - left - right));
    let height = height + top + bottom;
    let below = anchor.y + anchor.height + GAP;
    let y = if below + height > viewport[1] - GAP { (anchor.y - height - GAP).max(GAP) } else { below };
    let x = anchor.x.min(viewport[0] - width - GAP).max(GAP);
    let rect = Rect { x, y, width, height };
    let paint = &computed.paint;
    let shadows = |inset: bool| {
        paint.shadows.iter().filter(move |shadow| shadow.inset == inset).map(move |&shadow| Draw::Shadow {
            rect,
            radius: computed.radius,
            shadow,
        })
    };
    let content = Rect { x: x + left, y: y + top, width: width - left - right, height: height - top - bottom };
    let mut draws: Vec<Draw> = shadows(false).collect();
    draws.push(Draw::Rect { rect, fill: paint.fill, border: paint.border, radius: computed.radius });
    draws.extend(shadows(true));
    draws.push(Draw::Text { rect: content, text: tip.to_owned(), color: paint.color, style: computed.text.clone() });
    draws
}

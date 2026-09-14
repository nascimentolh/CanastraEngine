//! Style resolution and flex layout into draw commands.

use taffy::prelude::{
    AlignItems, AvailableSpace, Dimension, Display, FlexDirection, JustifyContent, LengthPercentage,
    LengthPercentageAuto, NodeId, Position, Rect as Edges, Size, Style, TaffyTree,
};

use crate::css::{Align, Declaration, Length, StyleSheet};
use crate::markup::{Element, Tag};
use crate::{Draw, Fill, Frame, Hit, Rect, Rgba, TextMeasure, UiError};

const DEFAULT_FONT_SIZE: f32 = 13.0;

/// What an element looks like after the cascade.
#[derive(Debug, Clone)]
struct Computed {
    fill: Option<Fill>,
    border: Option<(f32, Rgba)>,
    radius: f32,
    color: Rgba,
    font_size: f32,
    border_image: Option<(String, f32)>,
    padding: [f32; 4],
}

const ROOT: Computed = Computed {
    fill: None,
    border: None,
    radius: 0.0,
    color: Rgba([255; 4]),
    font_size: DEFAULT_FONT_SIZE,
    border_image: None,
    padding: [0.0; 4],
};

/// Text measured by taffy while laying out leaves.
struct Measured {
    text: String,
    size: f32,
}

struct Node<'a> {
    element: &'a Element,
    index: usize,
    id: NodeId,
    computed: Computed,
    text: Option<String>,
    children: Vec<Node<'a>>,
}

/// Lays out `ui` for a viewport and returns what to draw and where clicks land.
/// `hovered` is the pre-order index of the element under the pointer, for `:hover` rules.
pub fn build(
    ui: &Element,
    sheet: &StyleSheet,
    viewport: [f32; 2],
    hovered: Option<usize>,
    text: &mut dyn TextMeasure,
    bindings: &dyn Fn(&str) -> Option<String>,
) -> Result<Frame, UiError> {
    // ponytail: the layout tree is rebuilt every frame; cache it when screens get large.
    let mut tree: TaffyTree<Measured> = TaffyTree::new();
    let mut next_index = 0;
    let root = node(&mut tree, ui, sheet, hovered, &ROOT, &mut next_index, bindings).map_err(layout_error)?;
    let available =
        Size { width: AvailableSpace::Definite(viewport[0]), height: AvailableSpace::Definite(viewport[1]) };
    tree.compute_layout_with_measure(root.id, available, |input, _, context, style| {
        // Taffy applies the style's size, padding and limits; only the text content is measured here.
        taffy::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |known, available| match &context {
                Some(measured) => {
                    let max_width = known.width.or(match available.width {
                        AvailableSpace::Definite(width) => Some(width),
                        _ => None,
                    });
                    let (width, height) = text.measure(&measured.text, measured.size, max_width);
                    Size { width, height }
                }
                None => Size::ZERO,
            },
        )
    })
    .map_err(layout_error)?;

    let mut frame = Frame::default();
    emit(&tree, &root, 0.0, 0.0, &mut frame).map_err(layout_error)?;
    Ok(frame)
}

fn node<'a>(
    tree: &mut TaffyTree<Measured>,
    element: &'a Element,
    sheet: &StyleSheet,
    hovered: Option<usize>,
    parent: &Computed,
    next_index: &mut usize,
    bindings: &dyn Fn(&str) -> Option<String>,
) -> taffy::TaffyResult<Node<'a>> {
    let index = *next_index;
    *next_index += 1;
    let mut style = default_style(element.tag);
    // Only text color and size inherit; everything else starts over.
    let mut computed = Computed { color: parent.color, font_size: parent.font_size, ..ROOT };
    for declaration in sheet.cascade(element, hovered == Some(index)) {
        apply(declaration, &mut style, &mut computed);
    }

    let text = match element.tag {
        Tag::Label | Tag::Button => element.bind.as_deref().and_then(bindings).or_else(|| element.text.clone()),
        _ => None,
    };
    let (id, children) = if let Some(text) = &text {
        let measured = Measured { text: text.clone(), size: computed.font_size };
        (tree.new_leaf_with_context(style, measured)?, Vec::new())
    } else {
        let children = element
            .children
            .iter()
            .map(|child| node(tree, child, sheet, hovered, &computed, next_index, bindings))
            .collect::<taffy::TaffyResult<Vec<_>>>()?;
        let ids: Vec<NodeId> = children.iter().map(|child| child.id).collect();
        (tree.new_with_children(style, &ids)?, children)
    };
    Ok(Node { element, index, id, computed, text, children })
}

fn default_style(tag: Tag) -> Style {
    let mut style = Style { display: Display::Flex, ..Style::default() };
    match tag {
        Tag::Ui => style.size = Size { width: Dimension::percent(1.0), height: Dimension::percent(1.0) },
        Tag::Row => style.flex_direction = FlexDirection::Row,
        Tag::Window | Tag::Column => style.flex_direction = FlexDirection::Column,
        Tag::Label | Tag::Button | Tag::Image => {}
    }
    style
}

fn apply(declaration: &Declaration, style: &mut Style, computed: &mut Computed) {
    let px = LengthPercentage::length;
    let inset = |value: f32| LengthPercentageAuto::length(value);
    match declaration {
        Declaration::Width(length) => style.size.width = dimension(*length),
        Declaration::Height(length) => style.size.height = dimension(*length),
        Declaration::Padding([top, right, bottom, left]) => {
            style.padding = Edges { top: px(*top), right: px(*right), bottom: px(*bottom), left: px(*left) };
            computed.padding = [*top, *right, *bottom, *left];
        }
        Declaration::Margin([top, right, bottom, left]) => {
            style.margin = Edges { top: inset(*top), right: inset(*right), bottom: inset(*bottom), left: inset(*left) };
        }
        Declaration::Gap(gap) => style.gap = Size { width: px(*gap), height: px(*gap) },
        Declaration::Row(row) => {
            style.flex_direction = if *row { FlexDirection::Row } else { FlexDirection::Column };
        }
        Declaration::AlignItems(align) => style.align_items = Some(align_items(*align)),
        Declaration::JustifyContent(align) => style.justify_content = Some(justify(*align)),
        Declaration::FlexGrow(grow) => style.flex_grow = *grow,
        Declaration::Absolute(absolute) => {
            style.position = if *absolute { Position::Absolute } else { Position::Relative };
        }
        Declaration::Left(value) => style.inset.left = inset(*value),
        Declaration::Top(value) => style.inset.top = inset(*value),
        Declaration::Right(value) => style.inset.right = inset(*value),
        Declaration::Bottom(value) => style.inset.bottom = inset(*value),
        Declaration::Background(fill) => computed.fill = Some(*fill),
        Declaration::Border(border) => computed.border = *border,
        Declaration::BorderRadius(radius) => computed.radius = *radius,
        Declaration::Color(color) => computed.color = *color,
        Declaration::FontSize(size) => computed.font_size = *size,
        Declaration::BorderImage(source, inset) => computed.border_image = Some((source.clone(), *inset)),
    }
}

fn dimension(length: Length) -> Dimension {
    match length {
        Length::Auto => Dimension::auto(),
        Length::Px(value) => Dimension::length(value),
        Length::Percent(fraction) => Dimension::percent(fraction),
    }
}

fn align_items(align: Align) -> AlignItems {
    match align {
        Align::Start | Align::SpaceBetween => AlignItems::FLEX_START,
        Align::Center => AlignItems::CENTER,
        Align::End => AlignItems::FLEX_END,
        Align::Stretch => AlignItems::STRETCH,
    }
}

fn justify(align: Align) -> JustifyContent {
    match align {
        Align::Start | Align::Stretch => JustifyContent::FLEX_START,
        Align::Center => JustifyContent::CENTER,
        Align::End => JustifyContent::FLEX_END,
        Align::SpaceBetween => JustifyContent::SPACE_BETWEEN,
    }
}

fn emit(
    tree: &TaffyTree<Measured>,
    node: &Node<'_>,
    parent_x: f32,
    parent_y: f32,
    frame: &mut Frame,
) -> taffy::TaffyResult<()> {
    let layout = tree.layout(node.id)?;
    let rect = Rect {
        x: parent_x + layout.location.x,
        y: parent_y + layout.location.y,
        width: layout.size.width,
        height: layout.size.height,
    };
    let computed = &node.computed;
    if computed.fill.is_some() || computed.border.is_some() {
        frame.draws.push(Draw::Rect { rect, fill: computed.fill, border: computed.border, radius: computed.radius });
    }
    if let Some((source, inset)) = &computed.border_image {
        frame.draws.push(Draw::Image { rect, source: source.clone(), inset: *inset });
    }
    if let (Tag::Image, Some(source)) = (node.element.tag, &node.element.src) {
        frame.draws.push(Draw::Image { rect, source: source.clone(), inset: 0.0 });
    }
    if let Some(text) = &node.text {
        let [top, right, bottom, left] = computed.padding;
        let content = Rect {
            x: rect.x + left,
            y: rect.y + top,
            width: rect.width - left - right,
            height: rect.height - top - bottom,
        };
        frame.draws.push(Draw::Text {
            rect: content,
            text: text.clone(),
            color: computed.color,
            size: computed.font_size,
        });
    }
    if node.element.tag == Tag::Button || node.element.action.is_some() {
        frame.hits.push(Hit { rect, element: node.index, action: node.element.action.clone() });
    }
    for child in &node.children {
        emit(tree, child, rect.x, rect.y, frame)?;
    }
    Ok(())
}

fn layout_error(error: impl std::fmt::Display) -> UiError {
    UiError::Layout(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_markup, parse_stylesheet};

    /// Every character is 8 by 16 pixels, unwrapped.
    struct Monospace;

    impl TextMeasure for Monospace {
        fn measure(&mut self, text: &str, _size: f32, _max_width: Option<f32>) -> (f32, f32) {
            (f32::from(u16::try_from(text.chars().count()).unwrap()) * 8.0, 16.0)
        }
    }

    const MARKUP: &str = r#"<ui>
        <window id="login">
            <label bind="app.version" text="?"/>
            <button class="primary" text="Login" action="login"/>
        </window>
    </ui>"#;

    const CSS: &str = "
        #login { position: absolute; left: 100px; top: 50px; padding: 10px; gap: 4px; background: #202020; border: 1px solid #808080 }
        button { padding: 2px 6px; color: #ffffff }
        button:hover { color: #ffd700 }
    ";

    fn frame(hovered: Option<usize>) -> Frame {
        let ui = parse_markup(MARKUP).unwrap();
        let sheet = parse_stylesheet(CSS).unwrap();
        let bindings = |key: &str| (key == "app.version").then(|| "1.0".to_owned());
        build(&ui, &sheet, [800.0, 600.0], hovered, &mut Monospace, &bindings).unwrap()
    }

    #[test]
    fn lays_out_draws_and_hits() {
        let frame = frame(None);
        let window = Rect { x: 100.0, y: 50.0, width: 20.0 + 5.0 * 8.0 + 12.0, height: 20.0 + 16.0 + 4.0 + 20.0 };
        assert_eq!(
            frame.draws[0],
            Draw::Rect {
                rect: window,
                fill: Some(Fill::Solid(Rgba([0x20, 0x20, 0x20, 255]))),
                border: Some((1.0, Rgba([0x80, 0x80, 0x80, 255]))),
                radius: 0.0,
            }
        );
        assert!(matches!(&frame.draws[1], Draw::Text { text, .. } if text == "1.0"));
        let Draw::Text { rect, color, .. } = &frame.draws[2] else { panic!("{:?}", frame.draws) };
        assert_eq!((rect.x, rect.y, *color), (116.0, 82.0, Rgba([255; 4])));

        let hit = frame.hit(120.0, 85.0).unwrap();
        assert_eq!((hit.element, hit.action.as_deref()), (3, Some("login")));
        assert!(frame.hit(10.0, 10.0).is_none());
    }

    #[test]
    fn hover_restyles_the_hovered_element() {
        let Draw::Text { color, .. } = &frame(Some(3)).draws[2] else { panic!() };
        assert_eq!(*color, Rgba([255, 215, 0, 255]));
    }

    #[test]
    fn text_leaves_keep_their_styled_size() {
        let ui = parse_markup(r#"<ui><button text="Go" action="go"/></ui>"#).unwrap();
        let sheet = parse_stylesheet("ui { align-items: start } button { width: 100px; padding: 5px }").unwrap();
        let frame = build(&ui, &sheet, [800.0, 600.0], None, &mut Monospace, &|_| None).unwrap();
        assert_eq!(frame.hits[0].rect, Rect { x: 0.0, y: 0.0, width: 100.0, height: 26.0 });
    }
}

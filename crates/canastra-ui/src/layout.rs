//! Style resolution and flex layout into draw commands.

use taffy::prelude::{
    AlignItems, AvailableSpace, Dimension, Display, FlexDirection, JustifyContent, LengthPercentage,
    LengthPercentageAuto, NodeId, Position, Rect as Edges, Size, Style, TaffyTree,
};

use crate::css::{Align, Declaration, Length, States, StyleSheet};
use crate::markup::{Element, Tag};
use crate::transition::{Paint, Transitions};
use crate::{Draw, Fill, Frame, Hit, Rect, Rgba, TextAlign, TextMeasure, TextStyle, UiError, UiState};

const DEFAULT_FONT_SIZE: f32 = 13.0;
/// Width of an input's caret in logical pixels.
const CARET_WIDTH: f32 = 1.5;

/// What an element looks like after the cascade.
#[derive(Debug, Clone)]
struct Computed {
    paint: Paint,
    radius: f32,
    text: TextStyle,
    border_image: Option<(String, f32)>,
    padding: [f32; 4],
    /// Seconds paint changes take; 0 applies them at once.
    transition: f32,
}

const ROOT: Computed = Computed {
    paint: Paint { fill: None, border: None, shadows: Vec::new(), color: Rgba([255; 4]) },
    radius: 0.0,
    text: TextStyle { family: None, size: DEFAULT_FONT_SIZE, weight: 400, letter_spacing: 0.0, align: TextAlign::Left },
    border_image: None,
    padding: [0.0; 4],
    transition: 0.0,
};

/// Text measured by taffy while laying out leaves.
struct Measured {
    text: String,
    style: TextStyle,
}

struct Node<'a> {
    element: &'a Element,
    index: usize,
    id: NodeId,
    computed: Computed,
    text: Option<String>,
    /// An input's value as drawn (masked for passwords), which the caret follows.
    value: Option<String>,
    children: Vec<Node<'a>>,
}

/// What styling one frame reads and records, shared by every element.
struct Pass<'s> {
    sheet: &'s StyleSheet,
    state: UiState,
    transitions: &'s mut Transitions,
    bindings: &'s dyn Fn(&str) -> Option<String>,
    next_index: usize,
    animating: bool,
}

/// Lays out `ui` for a viewport and returns what to draw and where clicks land.
/// `transitions` carries running animations from frame to frame; start a new one for a new `ui`.
pub fn build(
    ui: &Element,
    sheet: &StyleSheet,
    viewport: [f32; 2],
    state: UiState,
    transitions: &mut Transitions,
    text: &mut dyn TextMeasure,
    bindings: &dyn Fn(&str) -> Option<String>,
) -> Result<Frame, UiError> {
    // ponytail: the layout tree is rebuilt every frame; cache it when screens get large.
    let mut tree: TaffyTree<Measured> = TaffyTree::new();
    let ui = expand(ui, bindings);
    let mut pass = Pass { sheet, state, transitions, bindings, next_index: 0, animating: false };
    let root = pass.node(&mut tree, &ui, &ROOT).map_err(layout_error)?;
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
                    let (width, height) = text.measure(&measured.text, &measured.style, max_width);
                    Size { width, height }
                }
                None => Size::ZERO,
            },
        )
    })
    .map_err(layout_error)?;

    let mut frame = Frame { animating: pass.animating, ..Frame::default() };
    let mut overlays = Vec::new();
    emit(&tree, &root, [0.0, 0.0], state, text, &mut frame, &mut overlays).map_err(layout_error)?;
    for overlay in overlays {
        // The renderer draws all text over all shapes, so text an overlay covers would show through it.
        // ponytail: covered text is dropped whole; draw layers apart if a label half under an overlay matters.
        let covered = |rect: &Rect| overlay.draws.iter().any(|draw| draw.rect().intersects(rect));
        frame.draws.retain(|draw| !matches!(draw, Draw::Text { rect, .. } if covered(rect)));
        frame.draws.extend(overlay.draws);
        frame.hits.extend(overlay.hits);
    }
    Ok(frame)
}

impl Pass<'_> {
    fn node<'a>(
        &mut self,
        tree: &mut TaffyTree<Measured>,
        element: &'a Element,
        parent: &Computed,
    ) -> taffy::TaffyResult<Node<'a>> {
        let index = self.next_index;
        self.next_index += 1;
        let bindings = self.bindings;
        let value = (element.tag == Tag::Input).then(|| {
            let value = element.bind.as_deref().and_then(bindings).unwrap_or_default();
            if element.password { "\u{2022}".repeat(value.chars().count()) } else { value }
        });
        let states = States {
            hover: self.state.hovered == Some(index),
            focus: self.state.focused == Some(index),
            empty: value.as_ref().is_some_and(String::is_empty),
        };
        let mut style = default_style(element.tag);
        // Only text color and style inherit; everything else starts over.
        let mut computed =
            Computed { paint: Paint { color: parent.paint.color, ..ROOT.paint }, text: parent.text.clone(), ..ROOT };
        for declaration in self.sheet.cascade(element, states) {
            apply(declaration, &mut style, &mut computed);
        }
        let (paint, moving) = self.transitions.paint(index, &computed.paint, computed.transition, self.state.time);
        computed.paint = paint;
        self.animating |= moving;

        let text = match (element.tag, &value) {
            (Tag::Label | Tag::Button, _) => {
                element.bind.as_deref().and_then(bindings).or_else(|| element.text.clone())
            }
            (Tag::Input, Some(value)) if value.is_empty() => Some(element.placeholder.clone().unwrap_or_default()),
            (Tag::Input, value) => value.clone(),
            _ => None,
        };
        let (id, children) = if let Some(text) = &text {
            let measured = Measured { text: text.clone(), style: computed.text.clone() };
            (tree.new_leaf_with_context(style, measured)?, Vec::new())
        } else {
            let children = element
                .children
                .iter()
                .map(|child| self.node(tree, child, &computed))
                .collect::<taffy::TaffyResult<Vec<_>>>()?;
            let ids: Vec<NodeId> = children.iter().map(|child| child.id).collect();
            (tree.new_with_children(style, &ids)?, children)
        };
        Ok(Node { element, index, id, computed, text, value, children })
    }
}

/// `element` with each `repeat` container's children copied once per item of its list.
fn expand(element: &Element, bindings: &dyn Fn(&str) -> Option<String>) -> Element {
    if element.tag == Tag::Select {
        let open = bindings(crate::OPEN_KEY).is_some_and(|key| element.bind.as_ref() == Some(&key));
        return expand(&crate::select::expand(element, open), bindings);
    }
    let children = match &element.repeat {
        Some(list) => {
            let count = bindings(&format!("{list}.len")).and_then(|len| len.parse::<usize>().ok()).unwrap_or(0);
            (0..count)
                .flat_map(|index| element.children.iter().map(move |child| instantiate(child, list, index)))
                .map(|child| expand(&child, bindings))
                .collect()
        }
        None => element.children.iter().map(|child| expand(child, bindings)).collect(),
    };
    Element { children, ..element.clone() }
}

/// A copy of a repeated `element` for item `index` of `list`.
fn instantiate(element: &Element, list: &str, index: usize) -> Element {
    let bind = element.bind.as_ref().map(|key| match key.strip_prefix("item.") {
        Some(field) => format!("{list}.{index}.{field}"),
        None => key.clone(),
    });
    let action = element.action.as_ref().map(|action| action.replace("{index}", &index.to_string()));
    let children = element.children.iter().map(|child| instantiate(child, list, index)).collect();
    Element { bind, action, children, ..element.clone() }
}

fn default_style(tag: Tag) -> Style {
    let mut style = Style { display: Display::Flex, ..Style::default() };
    match tag {
        Tag::Ui => style.size = Size { width: Dimension::percent(1.0), height: Dimension::percent(1.0) },
        Tag::Row => style.flex_direction = FlexDirection::Row,
        Tag::Window | Tag::Column => style.flex_direction = FlexDirection::Column,
        Tag::Label | Tag::Button | Tag::Image | Tag::Input | Tag::Select => {}
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
        Declaration::Background(fill) => computed.paint.fill = Some(*fill),
        Declaration::Border(border) => computed.paint.border = *border,
        Declaration::BorderRadius(radius) => computed.radius = *radius,
        Declaration::BoxShadow(shadows) => computed.paint.shadows.clone_from(shadows),
        Declaration::Color(color) => computed.paint.color = *color,
        Declaration::FontFamily(family) => computed.text.family = Some(family.clone()),
        Declaration::FontSize(size) => computed.text.size = *size,
        Declaration::FontWeight(weight) => computed.text.weight = *weight,
        Declaration::LetterSpacing(spacing) => computed.text.letter_spacing = *spacing,
        Declaration::TextAlign(align) => computed.text.align = *align,
        Declaration::BorderImage(source, inset) => computed.border_image = Some((source.clone(), *inset)),
        Declaration::Transition(seconds) => computed.transition = *seconds,
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
    [parent_x, parent_y]: [f32; 2],
    state: UiState,
    text: &mut dyn TextMeasure,
    frame: &mut Frame,
    overlays: &mut Vec<Frame>,
) -> taffy::TaffyResult<()> {
    let layout = tree.layout(node.id)?;
    let rect = Rect {
        x: parent_x + layout.location.x,
        y: parent_y + layout.location.y,
        width: layout.size.width,
        height: layout.size.height,
    };
    let computed = &node.computed;
    let paint = &computed.paint;
    let shadows = |inset: bool| {
        paint.shadows.iter().filter(move |shadow| shadow.inset == inset).map(|&shadow| Draw::Shadow {
            rect,
            radius: computed.radius,
            shadow,
        })
    };
    frame.draws.extend(shadows(false));
    if paint.fill.is_some() || paint.border.is_some() {
        frame.draws.push(Draw::Rect { rect, fill: paint.fill, border: paint.border, radius: computed.radius });
    }
    if let Some((source, inset)) = &computed.border_image {
        frame.draws.push(Draw::Image { rect, source: source.clone(), inset: *inset });
    }
    if let (Tag::Image, Some(source)) = (node.element.tag, &node.element.src) {
        frame.draws.push(Draw::Image { rect, source: source.clone(), inset: 0.0 });
    }
    frame.draws.extend(shadows(true));
    if let Some(label) = &node.text {
        let [top, right, bottom, left] = computed.padding;
        let content = Rect {
            x: rect.x + left,
            y: rect.y + top,
            width: rect.width - left - right,
            height: rect.height - top - bottom,
        };
        frame.draws.push(Draw::Text {
            rect: content,
            text: label.clone(),
            color: paint.color,
            style: computed.text.clone(),
        });
        // ponytail: the caret sits after the last character; add a cursor position when editing mid-text is needed.
        if let Some(value) = node.value.as_ref().filter(|_| state.caret && state.focused == Some(node.index)) {
            let (width, _) = text.measure(value, &computed.text, None);
            let caret = Rect { x: content.x + width, width: CARET_WIDTH, ..content };
            frame.draws.push(Draw::Rect {
                rect: caret,
                fill: Some(Fill::Solid(paint.color)),
                border: None,
                radius: 0.0,
            });
        }
    }
    let element = node.element;
    if matches!(element.tag, Tag::Button | Tag::Input) || element.action.is_some() {
        let field = element.bind.clone().filter(|_| element.tag == Tag::Input);
        frame.hits.push(Hit { rect, element: node.index, action: element.action.clone(), field });
    }
    for child in &node.children {
        if child.element.overlay {
            let mut overlay = Frame::default();
            emit(tree, child, [rect.x, rect.y], state, text, &mut overlay, overlays)?;
            overlays.push(overlay);
        } else {
            emit(tree, child, [rect.x, rect.y], state, text, frame, overlays)?;
        }
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
        fn measure(&mut self, text: &str, _style: &TextStyle, _max_width: Option<f32>) -> (f32, f32) {
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

    fn frame(state: UiState) -> Frame {
        let ui = parse_markup(MARKUP).unwrap();
        let sheet = parse_stylesheet(CSS).unwrap();
        let bindings = |key: &str| (key == "app.version").then(|| "1.0".to_owned());
        build(&ui, &sheet, [800.0, 600.0], state, &mut Transitions::default(), &mut Monospace, &bindings).unwrap()
    }

    #[test]
    fn lays_out_draws_and_hits() {
        let frame = frame(UiState::default());
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
        let Draw::Text { color, .. } = &frame(UiState { hovered: Some(3), ..UiState::default() }).draws[2] else {
            panic!()
        };
        assert_eq!(*color, Rgba([255, 215, 0, 255]));
    }

    #[test]
    fn text_leaves_keep_their_styled_size() {
        let ui = parse_markup(r#"<ui><button text="Go" action="go"/></ui>"#).unwrap();
        let sheet = parse_stylesheet("ui { align-items: start } button { width: 100px; padding: 5px }").unwrap();
        let frame = build(
            &ui,
            &sheet,
            [800.0, 600.0],
            UiState::default(),
            &mut Transitions::default(),
            &mut Monospace,
            &|_| None,
        )
        .unwrap();
        assert_eq!(frame.hits[0].rect, Rect { x: 0.0, y: 0.0, width: 100.0, height: 26.0 });
    }

    #[test]
    fn inputs_show_placeholders_masks_and_a_caret() {
        let ui = parse_markup(
            r#"<ui><input bind="account" placeholder="Account"/><input bind="password" type="password" action="login"/></ui>"#,
        )
        .unwrap();
        let sheet =
            parse_stylesheet("ui { align-items: start } input:empty { color: #888 } input:focus { color: #ff0 }")
                .unwrap();
        let bindings = |key: &str| (key == "password").then(|| "abc".to_owned());
        let state = UiState { focused: Some(2), caret: true, ..UiState::default() };
        let frame =
            build(&ui, &sheet, [800.0, 600.0], state, &mut Transitions::default(), &mut Monospace, &bindings).unwrap();

        let Draw::Text { text, color, .. } = &frame.draws[0] else { panic!("{:?}", frame.draws) };
        assert_eq!((text.as_str(), *color), ("Account", Rgba([0x88, 0x88, 0x88, 255])));
        let Draw::Text { text, rect, .. } = &frame.draws[1] else { panic!("{:?}", frame.draws) };
        assert_eq!(text, "\u{2022}\u{2022}\u{2022}");
        let yellow = Some(Fill::Solid(Rgba([255, 255, 0, 255])));
        assert_eq!(
            frame.draws[2],
            Draw::Rect {
                rect: Rect { x: rect.x + 24.0, width: CARET_WIDTH, ..*rect },
                fill: yellow,
                border: None,
                radius: 0.0
            }
        );

        let fields: Vec<_> =
            frame.fields().map(|hit| (hit.element, hit.field.as_deref(), hit.action.as_deref())).collect();
        assert_eq!(fields, [(1, Some("account"), None), (2, Some("password"), Some("login"))]);
    }

    #[test]
    fn an_open_select_lists_its_options_over_what_follows() {
        let ui = parse_markup(
            r#"<ui><select bind="race" options="races" action="race:{index}"/><button text="Next" action="next"/></ui>"#,
        )
        .unwrap();
        let sheet = parse_stylesheet(
            "ui { flex-direction: column; align-items: start } .select-options { position: absolute; top: 16px }",
        )
        .unwrap();
        let frame = |open: bool| {
            let bindings = move |key: &str| match key {
                "race" => Some("Dwarf".to_owned()),
                "races.len" => Some("2".to_owned()),
                "races.0.name" => Some("Human".to_owned()),
                "races.1.name" => Some("Elf".to_owned()),
                crate::OPEN_KEY if open => Some("race".to_owned()),
                _ => None,
            };
            let state = UiState::default();
            build(&ui, &sheet, [800.0, 600.0], state, &mut Transitions::default(), &mut Monospace, &bindings).unwrap()
        };
        let actions = |frame: &Frame| frame.hits.iter().filter_map(|hit| hit.action.clone()).collect::<Vec<_>>();

        assert_eq!(actions(&frame(false)), ["ui.open:race", "next"]);
        let open = frame(true);
        assert_eq!(actions(&open), ["ui.open:race", "next", "race:0", "race:1"]);
        let next = open.hits.iter().find(|hit| hit.action.as_deref() == Some("next")).unwrap().rect;
        let option = open.hits.iter().find(|hit| hit.action.as_deref() == Some("race:0")).unwrap().rect;
        assert!(option.contains(next.x + 1.0, next.y + 1.0), "the options cover the button after the select");
        assert_eq!(open.hit(next.x + 1.0, next.y + 1.0).unwrap().action.as_deref(), Some("race:0"));
        let texts: Vec<_> = open
            .draws
            .iter()
            .filter_map(|draw| if let Draw::Text { text, .. } = draw { Some(text.as_str()) } else { None })
            .collect();
        assert!(!texts.contains(&"Next"), "text under the options is hidden: {texts:?}");
    }

    #[test]
    fn repeated_children_bind_each_item() {
        let ui = parse_markup(
            r#"<ui><column repeat="servers"><button bind="item.name" action="pick:{index}"/></column></ui>"#,
        )
        .unwrap();
        let sheet = parse_stylesheet("ui { align-items: start }").unwrap();
        let bindings = |key: &str| match key {
            "servers.len" => Some("2".to_owned()),
            "servers.0.name" => Some("Aden".to_owned()),
            "servers.1.name" => Some("Giran".to_owned()),
            _ => None,
        };
        let frame = build(
            &ui,
            &sheet,
            [800.0, 600.0],
            UiState::default(),
            &mut Transitions::default(),
            &mut Monospace,
            &bindings,
        )
        .unwrap();
        let actions: Vec<_> = frame.hits.iter().map(|hit| hit.action.as_deref()).collect();
        assert_eq!(actions, [Some("pick:0"), Some("pick:1")]);
        let texts: Vec<_> = frame
            .draws
            .iter()
            .filter_map(|draw| if let Draw::Text { text, .. } = draw { Some(text.as_str()) } else { None })
            .collect();
        assert_eq!(texts, ["Aden", "Giran"]);
    }
}

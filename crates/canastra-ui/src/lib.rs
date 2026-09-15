//! Canastra UI core: markup plus a CSS subset, laid out into a draw list.
//!
//! No GPU and no fonts: text size comes from a [`TextMeasure`] the renderer provides, and the
//! result is plain draw commands and hit areas, so every rule here is testable without a window.

mod css;
mod layout;
mod markup;
mod select;
mod transition;

use std::fmt;
use std::sync::Arc;

pub use css::{StyleSheet, parse_stylesheet};
pub use layout::build;
pub use markup::{Element, Tag, parse_markup};
pub use select::{OPEN_ACTION, OPEN_KEY};
pub use transition::Transitions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgba(pub [u8; 4]);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fill {
    Solid(Rgba),
    /// Top color to bottom color.
    Vertical(Rgba, Rgba),
    /// Center color to the color at the corners.
    Radial(Rgba, Rgba),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// How text is shaped; inherited from parent to child like in CSS.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Font family name; `None` is the platform's sans-serif.
    pub family: Option<Arc<str>>,
    /// Font size in logical pixels.
    pub size: f32,
    /// 100 to 900; 400 is normal, 700 bold.
    pub weight: u16,
    /// Extra space between letters in logical pixels.
    pub letter_spacing: f32,
    pub align: TextAlign,
}

/// One `box-shadow` layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub offset: [f32; 2],
    /// Distance over which the shadow fades.
    pub blur: f32,
    pub color: Rgba,
    /// Drawn inside the shape, over its background, instead of behind it.
    pub inset: bool,
}

/// Logical pixels from the top-left of the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    pub(crate) fn intersects(&self, other: &Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Draw {
    /// A shadow of the rounded rectangle `rect`: outside it, or clipped inside it when inset.
    Shadow {
        rect: Rect,
        radius: f32,
        shadow: Shadow,
    },
    Rect {
        rect: Rect,
        fill: Option<Fill>,
        border: Option<(f32, Rgba)>,
        radius: f32,
    },
    /// A texture; with `inset > 0` its edges keep their size and only the middle stretches.
    Image {
        rect: Rect,
        source: String,
        inset: f32,
    },
    Text {
        rect: Rect,
        text: String,
        color: Rgba,
        style: TextStyle,
    },
}

impl Draw {
    pub(crate) fn rect(&self) -> &Rect {
        match self {
            Self::Shadow { rect, .. }
            | Self::Rect { rect, .. }
            | Self::Image { rect, .. }
            | Self::Text { rect, .. } => rect,
        }
    }
}

/// An interactive element on screen, in pre-order element index.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub rect: Rect,
    pub element: usize,
    pub action: Option<String>,
    /// For an input, the data key it edits.
    pub field: Option<String>,
}

/// Interaction that restyles a screen between frames without changing its markup.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiState {
    /// Pre-order index of the element under the pointer, for `:hover`.
    pub hovered: Option<usize>,
    /// Pre-order index of the input receiving keys, for `:focus` and its caret.
    pub focused: Option<usize>,
    /// Whether the focused input's blinking caret is visible this frame.
    pub caret: bool,
    /// Seconds on any steady clock, for transitions.
    pub time: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    /// Back to front.
    pub draws: Vec<Draw>,
    hits: Vec<Hit>,
    /// A transition is still running, so the next frame will differ.
    pub animating: bool,
}

impl Frame {
    /// The topmost interactive element under the point.
    pub fn hit(&self, x: f32, y: f32) -> Option<&Hit> {
        self.hits.iter().rev().find(|hit| hit.rect.contains(x, y))
    }

    /// Inputs in document order, for moving focus.
    pub fn fields(&self) -> impl Iterator<Item = &Hit> {
        self.hits.iter().filter(|hit| hit.field.is_some())
    }
}

/// Measures text as the renderer will draw it.
pub trait TextMeasure {
    /// Width and height of `text` in `style`, wrapped to `max_width` when given.
    fn measure(&mut self, text: &str, style: &TextStyle, max_width: Option<f32>) -> (f32, f32);
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiError {
    Markup(String),
    UnknownTag(String),
    UnknownAttribute { tag: String, attribute: String },
    Css(String),
    Layout(String),
}

impl fmt::Display for UiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Markup(message) => write!(f, "markup: {message}"),
            Self::UnknownTag(tag) => write!(f, "markup: unknown element <{tag}>"),
            Self::UnknownAttribute { tag, attribute } => write!(f, "markup: <{tag}> has no attribute `{attribute}`"),
            Self::Css(message) => write!(f, "css: {message}"),
            Self::Layout(message) => write!(f, "layout: {message}"),
        }
    }
}

impl std::error::Error for UiError {}

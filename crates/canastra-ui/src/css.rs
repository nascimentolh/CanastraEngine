//! The CSS subset: simple compound selectors, a fixed set of properties, strict values.

use crate::markup::{Element, Tag};
use crate::{Fill, Rgba, Shadow, TextAlign, UiError};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleSheet {
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
struct Rule {
    selector: Selector,
    declarations: Vec<Declaration>,
}

/// `tag`, `.class`, `#id` and `:hover`, combined without spaces, e.g. `button.primary:hover`.
#[derive(Debug, Clone, Default, PartialEq)]
struct Selector {
    tag: Option<Tag>,
    id: Option<String>,
    classes: Vec<String>,
    hover: bool,
}

impl Selector {
    fn matches(&self, element: &Element, hovered: bool) -> bool {
        self.tag.is_none_or(|tag| tag == element.tag)
            && self.id.as_ref().is_none_or(|id| element.id.as_ref() == Some(id))
            && self.classes.iter().all(|class| element.classes.contains(class))
            && (!self.hover || hovered)
    }

    /// Ids, then classes and states, then tags.
    fn specificity(&self) -> (usize, usize, usize) {
        (usize::from(self.id.is_some()), self.classes.len() + usize::from(self.hover), usize::from(self.tag.is_some()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Length {
    Auto,
    Px(f32),
    Percent(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Align {
    Start,
    Center,
    End,
    Stretch,
    SpaceBetween,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Declaration {
    Width(Length),
    Height(Length),
    /// Top, right, bottom, left.
    Padding([f32; 4]),
    Margin([f32; 4]),
    Gap(f32),
    Row(bool),
    AlignItems(Align),
    JustifyContent(Align),
    FlexGrow(f32),
    Absolute(bool),
    Left(f32),
    Top(f32),
    Right(f32),
    Bottom(f32),
    Background(Fill),
    Border(Option<(f32, Rgba)>),
    BorderRadius(f32),
    BoxShadow(Vec<Shadow>),
    Color(Rgba),
    FontSize(f32),
    FontWeight(u16),
    LetterSpacing(f32),
    TextAlign(TextAlign),
    /// Texture path and the inset, in texture pixels, kept unstretched at each edge.
    BorderImage(String, f32),
}

impl StyleSheet {
    /// Declarations that apply to `element`, weakest first, so later ones win when applied in order.
    pub(crate) fn cascade(&self, element: &Element, hovered: bool) -> Vec<&Declaration> {
        let mut matching: Vec<(usize, &Rule)> =
            self.rules.iter().enumerate().filter(|(_, rule)| rule.selector.matches(element, hovered)).collect();
        matching.sort_by_key(|(order, rule)| (rule.selector.specificity(), *order));
        matching.into_iter().flat_map(|(_, rule)| &rule.declarations).collect()
    }
}

pub fn parse_stylesheet(source: &str) -> Result<StyleSheet, UiError> {
    let source = strip_comments(source);
    let mut rules = Vec::new();
    let mut rest = source.as_str();
    while let Some(open) = rest.find('{') {
        let close = rest[open..].find('}').map(|offset| open + offset).ok_or_else(|| css("unclosed `{`"))?;
        let declarations = rest[open + 1..close]
            .split(';')
            .filter(|text| !text.trim().is_empty())
            .map(declaration)
            .collect::<Result<Vec<_>, _>>()?;
        for selector in rest[..open].split(',') {
            rules.push(Rule { selector: self::selector(selector.trim())?, declarations: declarations.clone() });
        }
        rest = &rest[close + 1..];
    }
    if !rest.trim().is_empty() {
        return Err(css(&format!("unexpected `{}`", rest.trim())));
    }
    Ok(StyleSheet { rules })
}

fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = rest[start..].find("*/").map_or("", |end| &rest[start + end + 2..]);
    }
    out.push_str(rest);
    out
}

fn selector(text: &str) -> Result<Selector, UiError> {
    let mut selector = Selector::default();
    let word_end = |text: &str| text.find(['.', '#', ':']).unwrap_or(text.len());
    let tag_end = word_end(text);
    if tag_end > 0 {
        let name = &text[..tag_end];
        selector.tag = Some(Tag::from_name(name).ok_or_else(|| css(&format!("unknown element `{name}`")))?);
    }
    let mut rest = &text[tag_end..];
    while let Some(marker) = rest.chars().next() {
        let body = &rest[1..];
        let end = word_end(body);
        let name = &body[..end];
        if name.is_empty() || name.contains(char::is_whitespace) {
            return Err(css(&format!("unsupported selector `{text}`")));
        }
        match (marker, name) {
            ('.', _) => selector.classes.push(name.to_owned()),
            ('#', _) => selector.id = Some(name.to_owned()),
            (':', "hover") => selector.hover = true,
            _ => return Err(css(&format!("unsupported selector `{text}`"))),
        }
        rest = &body[end..];
    }
    if selector == Selector::default() {
        return Err(css("empty selector"));
    }
    Ok(selector)
}

fn declaration(text: &str) -> Result<Declaration, UiError> {
    let (name, value) =
        text.split_once(':').ok_or_else(|| css(&format!("expected `name: value` in `{}`", text.trim())))?;
    let value = value.trim();
    let px = || pixels(value);
    Ok(match name.trim() {
        "width" => Declaration::Width(length(value)?),
        "height" => Declaration::Height(length(value)?),
        "padding" => Declaration::Padding(edges(value)?),
        "margin" => Declaration::Margin(edges(value)?),
        "gap" => Declaration::Gap(px()?),
        "flex-direction" => Declaration::Row(keyword(value, &[("row", true), ("column", false)])?),
        "align-items" => Declaration::AlignItems(align(value)?),
        "justify-content" => Declaration::JustifyContent(align(value)?),
        "flex-grow" => Declaration::FlexGrow(number(value)?),
        "position" => Declaration::Absolute(keyword(value, &[("absolute", true), ("relative", false)])?),
        "left" => Declaration::Left(px()?),
        "top" => Declaration::Top(px()?),
        "right" => Declaration::Right(px()?),
        "bottom" => Declaration::Bottom(px()?),
        "background" => Declaration::Background(fill(value)?),
        "border" => Declaration::Border(border(value)?),
        "border-radius" => Declaration::BorderRadius(px()?),
        "box-shadow" => Declaration::BoxShadow(shadows(value)?),
        "color" => Declaration::Color(color(value)?),
        "font-size" => Declaration::FontSize(px()?),
        "font-weight" => Declaration::FontWeight(weight(value)?),
        "letter-spacing" => Declaration::LetterSpacing(px()?),
        "text-align" => Declaration::TextAlign(keyword(
            value,
            &[("left", TextAlign::Left), ("center", TextAlign::Center), ("right", TextAlign::Right)],
        )?),
        "border-image" => border_image(value)?,
        other => return Err(css(&format!("unsupported property `{other}`"))),
    })
}

fn length(value: &str) -> Result<Length, UiError> {
    if value == "auto" {
        return Ok(Length::Auto);
    }
    match value.strip_suffix('%') {
        Some(percent) => Ok(Length::Percent(number(percent)? / 100.0)),
        None => Ok(Length::Px(pixels(value)?)),
    }
}

fn pixels(value: &str) -> Result<f32, UiError> {
    match value {
        "0" => Ok(0.0),
        _ => number(value.strip_suffix("px").ok_or_else(|| css(&format!("expected pixels, found `{value}`")))?),
    }
}

fn number(value: &str) -> Result<f32, UiError> {
    value.trim().parse().map_err(|_| css(&format!("expected a number, found `{value}`")))
}

/// One to four pixel values, in CSS order.
fn edges(value: &str) -> Result<[f32; 4], UiError> {
    let values = value.split_whitespace().map(pixels).collect::<Result<Vec<_>, _>>()?;
    Ok(match *values.as_slice() {
        [all] => [all; 4],
        [vertical, horizontal] => [vertical, horizontal, vertical, horizontal],
        [top, horizontal, bottom] => [top, horizontal, bottom, horizontal],
        [top, right, bottom, left] => [top, right, bottom, left],
        _ => return Err(css(&format!("expected one to four lengths, found `{value}`"))),
    })
}

/// `normal`, `bold` or a multiple of 100 from 100 to 900.
fn weight(value: &str) -> Result<u16, UiError> {
    match value {
        "normal" => Ok(400),
        "bold" => Ok(700),
        _ => value
            .parse()
            .ok()
            .filter(|weight| (100..=900).contains(weight) && weight % 100 == 0)
            .ok_or_else(|| css(&format!("expected normal, bold or 100..900, found `{value}`"))),
    }
}

fn keyword<T: Copy>(value: &str, options: &[(&str, T)]) -> Result<T, UiError> {
    options.iter().find(|(name, _)| *name == value).map(|&(_, option)| option).ok_or_else(|| {
        let names: Vec<&str> = options.iter().map(|(name, _)| *name).collect();
        css(&format!("expected one of {}, found `{value}`", names.join(", ")))
    })
}

fn align(value: &str) -> Result<Align, UiError> {
    keyword(
        value,
        &[
            ("start", Align::Start),
            ("center", Align::Center),
            ("end", Align::End),
            ("stretch", Align::Stretch),
            ("space-between", Align::SpaceBetween),
        ],
    )
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
pub(crate) fn color(value: &str) -> Result<Rgba, UiError> {
    let invalid = || css(&format!("expected a #hex color, found `{value}`"));
    let hex = value.strip_prefix('#').ok_or_else(invalid)?;
    let digits = hex
        .chars()
        .map(|digit| digit.to_digit(16).and_then(|d| u8::try_from(d).ok()))
        .collect::<Option<Vec<u8>>>()
        .ok_or_else(invalid)?;
    let channels: Vec<u8> = match digits.len() {
        3 | 4 => digits.iter().map(|&d| d * 17).collect(),
        6 | 8 => digits.as_chunks::<2>().0.iter().map(|&[high, low]| high * 16 + low).collect(),
        _ => return Err(invalid()),
    };
    Ok(match *channels.as_slice() {
        [r, g, b] => Rgba([r, g, b, 255]),
        [r, g, b, a] => Rgba([r, g, b, a]),
        _ => return Err(invalid()),
    })
}

/// A color, `linear-gradient(<top>, <bottom>)` or `radial-gradient(<center>, <corners>)`.
fn fill(value: &str) -> Result<Fill, UiError> {
    let gradient = |name: &str| value.strip_prefix(name)?.strip_prefix('(')?.strip_suffix(')');
    let colors = |inner: &str| {
        let (first, second) = inner.split_once(',').ok_or_else(|| css("a gradient takes two colors"))?;
        Ok::<_, UiError>((color(first.trim())?, color(second.trim())?))
    };
    if let Some(inner) = gradient("linear-gradient") {
        let (top, bottom) = colors(inner)?;
        return Ok(Fill::Vertical(top, bottom));
    }
    if let Some(inner) = gradient("radial-gradient") {
        let (center, corners) = colors(inner)?;
        return Ok(Fill::Radial(center, corners));
    }
    Ok(Fill::Solid(color(value)?))
}

/// `none` or comma-separated `[inset] <x> <y> <blur> <color>` layers.
fn shadows(value: &str) -> Result<Vec<Shadow>, UiError> {
    if value == "none" {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|layer| {
            let words: Vec<&str> = layer.split_whitespace().collect();
            let (inset, words) = match words.as_slice() {
                ["inset", rest @ ..] => (true, rest),
                rest => (false, rest),
            };
            let [x, y, blur, paint] = words else {
                return Err(css(&format!("expected `[inset] <x> <y> <blur> <color>`, found `{}`", layer.trim())));
            };
            Ok(Shadow { offset: [pixels(x)?, pixels(y)?], blur: pixels(blur)?.max(0.0), color: color(paint)?, inset })
        })
        .collect()
}

/// `none` or `<width>px solid <color>`.
fn border(value: &str) -> Result<Option<(f32, Rgba)>, UiError> {
    match value.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["none"] => Ok(None),
        [width, "solid", paint] => Ok(Some((pixels(width)?, color(paint)?))),
        _ => Err(css(&format!("expected `none` or `<width> solid <color>`, found `{value}`"))),
    }
}

/// `url(Package.Group.Name) <inset>`.
fn border_image(value: &str) -> Result<Declaration, UiError> {
    let invalid = || css(&format!("expected `url(<texture>) <inset>`, found `{value}`"));
    let inner = value.strip_prefix("url(").ok_or_else(invalid)?;
    let (source, inset) = inner.split_once(')').ok_or_else(invalid)?;
    Ok(Declaration::BorderImage(source.trim().to_owned(), number(inset)?))
}

fn css(message: &str) -> UiError {
    UiError::Css(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markup::parse_markup;

    #[test]
    #[expect(clippy::float_cmp, reason = "values are parsed from exact literals")]
    fn parses_values() {
        assert_eq!(color("#f80").unwrap(), Rgba([255, 136, 0, 255]));
        assert_eq!(color("#11223344").unwrap(), Rgba([0x11, 0x22, 0x33, 0x44]));
        assert!(color("red").is_err() && color("#12345").is_err());
        assert_eq!(edges("1px 2px").unwrap(), [1.0, 2.0, 1.0, 2.0]);
        assert_eq!(length("50%").unwrap(), Length::Percent(0.5));
        assert_eq!(fill("linear-gradient(#000, #fff)").unwrap(), Fill::Vertical(Rgba([0, 0, 0, 255]), Rgba([255; 4])));
        assert_eq!(fill("radial-gradient(#fff, #000)").unwrap(), Fill::Radial(Rgba([255; 4]), Rgba([0, 0, 0, 255])));
        assert_eq!((weight("bold").unwrap(), weight("300").unwrap()), (700, 300));
        assert!(weight("350").is_err() && weight("1000").is_err());
        assert_eq!(border("1px solid #fff").unwrap(), Some((1.0, Rgba([255; 4]))));
        assert_eq!(
            shadows("0 -4px 12px #0008, inset 0 1px 0 #fff").unwrap(),
            [
                Shadow { offset: [0.0, -4.0], blur: 12.0, color: Rgba([0, 0, 0, 0x88]), inset: false },
                Shadow { offset: [0.0, 1.0], blur: 0.0, color: Rgba([255; 4]), inset: true },
            ]
        );
        assert!(shadows("none").unwrap().is_empty() && shadows("1px 2px #000").is_err());
        assert_eq!(
            declaration("border-image: url(L2UI_CH3.Button.Btn1_normal) 4").unwrap(),
            Declaration::BorderImage("L2UI_CH3.Button.Btn1_normal".into(), 4.0)
        );
    }

    #[test]
    fn rejects_what_it_does_not_support() {
        for source in
            ["div { color: #fff }", "button { float: left }", "window row { gap: 1px }", "label { color: #fff"]
        {
            assert!(parse_stylesheet(source).is_err(), "{source}");
        }
    }

    #[test]
    fn cascades_by_specificity_then_order() {
        let sheet = parse_stylesheet(
            "/* theme */ #ok { color: #00f } button.primary { color: #0f0 } button { color: #f00 } button:hover { font-size: 20px }",
        )
        .unwrap();
        let ui = parse_markup(r#"<ui><button id="ok" class="primary"/></ui>"#).unwrap();
        let button = &ui.children[0];
        let colors: Vec<_> = sheet.cascade(button, false).into_iter().cloned().collect();
        assert_eq!(
            colors,
            [
                Declaration::Color(Rgba([255, 0, 0, 255])),
                Declaration::Color(Rgba([0, 255, 0, 255])),
                Declaration::Color(Rgba([0, 0, 255, 255])),
            ]
        );
        assert_eq!(sheet.cascade(button, true).len(), 4);
    }
}

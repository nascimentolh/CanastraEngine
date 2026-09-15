//! UI markup: a small XML vocabulary, validated strictly so editors and skins fail loudly.

use roxmltree::{Document, Node};

use crate::UiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Ui,
    Window,
    Row,
    Column,
    Label,
    Button,
    Image,
    Input,
    /// A value picked from a list, opened below it; see `crate::select`.
    Select,
}

impl Tag {
    const ALL: [(&'static str, Self); 9] = [
        ("ui", Self::Ui),
        ("window", Self::Window),
        ("row", Self::Row),
        ("column", Self::Column),
        ("label", Self::Label),
        ("button", Self::Button),
        ("image", Self::Image),
        ("input", Self::Input),
        ("select", Self::Select),
    ];

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().find(|(tag, _)| *tag == name).map(|&(_, tag)| tag)
    }

    fn is_leaf(self) -> bool {
        matches!(self, Self::Label | Self::Button | Self::Image | Self::Input | Self::Select)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub tag: Tag,
    pub id: Option<String>,
    pub classes: Vec<String>,
    /// Literal text of a label or button.
    pub text: Option<String>,
    /// Data key whose value replaces `text`, e.g. `app.version`; for an input, the key it edits.
    pub bind: Option<String>,
    /// Data key that puts the element in `:checked` while it holds `true`, e.g. the selected item of a list.
    pub checked: Option<String>,
    /// Action name reported when the element is clicked, or when Enter is pressed in an input.
    pub action: Option<String>,
    /// Text an empty input shows.
    pub placeholder: Option<String>,
    /// An input whose value is drawn masked (`type="password"`).
    pub password: bool,
    /// Texture path of an image, e.g. `L2UI_CH3.Button.Btn1_normal`.
    pub src: Option<String>,
    /// On a container, the data list its children repeat for: once per item, with `item.` keys bound to
    /// that item and `{index}` in actions replaced by its position. The count comes from `<list>.len`.
    pub repeat: Option<String>,
    /// On a select, the data list it picks from, bound like `repeat`: `<list>.len` and `<list>.<index>.name`.
    pub options: Option<String>,
    /// Drawn after everything else and hit first, like an open select's options; set by expansion only.
    pub overlay: bool,
    pub children: Vec<Element>,
}

impl Element {
    pub(crate) fn new(tag: Tag) -> Self {
        Self {
            tag,
            id: None,
            classes: Vec::new(),
            text: None,
            bind: None,
            checked: None,
            action: None,
            placeholder: None,
            password: false,
            src: None,
            repeat: None,
            options: None,
            overlay: false,
            children: Vec::new(),
        }
    }
}

pub fn parse_markup(source: &str) -> Result<Element, UiError> {
    let document = Document::parse(source).map_err(|error| UiError::Markup(error.to_string()))?;
    let root = element(document.root_element())?;
    match root.tag {
        Tag::Ui => Ok(root),
        _ => Err(UiError::Markup("the root element must be <ui>".into())),
    }
}

fn element(node: Node<'_, '_>) -> Result<Element, UiError> {
    let name = node.tag_name().name();
    let tag = Tag::from_name(name).ok_or_else(|| UiError::UnknownTag(name.to_owned()))?;
    let mut element = Element::new(tag);
    for attribute in node.attributes() {
        let value = attribute.value().to_owned();
        match attribute.name() {
            "id" => element.id = Some(value),
            "class" => element.classes = value.split_whitespace().map(str::to_owned).collect(),
            "text" => element.text = Some(value),
            "bind" => element.bind = Some(value),
            "action" => element.action = Some(value),
            "checked" => element.checked = Some(value),
            "src" => element.src = Some(value),
            "placeholder" => element.placeholder = Some(value),
            "repeat" if !tag.is_leaf() => element.repeat = Some(value),
            "options" if tag == Tag::Select => element.options = Some(value),
            "type" => {
                element.password = match value.as_str() {
                    "text" => false,
                    "password" => true,
                    _ => return Err(UiError::Markup(format!("<{name}> type must be text or password, not `{value}`"))),
                }
            }
            other => return Err(UiError::UnknownAttribute { tag: name.to_owned(), attribute: other.to_owned() }),
        }
    }
    for child in node.children() {
        if child.is_text() && child.text().is_some_and(|text| !text.trim().is_empty()) {
            return Err(UiError::Markup(format!("<{name}> holds text; use the text attribute")));
        }
        if child.is_element() {
            if tag.is_leaf() {
                return Err(UiError::Markup(format!("<{name}> cannot have children")));
            }
            element.children.push(self::element(child)?);
        }
    }
    Ok(element)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_rejects_strictly() {
        let ui = parse_markup(
            r#"<ui><window id="login" class="panel dark"><button text="OK" action="login"/></window></ui>"#,
        )
        .unwrap();
        let window = &ui.children[0];
        assert_eq!((window.tag, window.id.as_deref()), (Tag::Window, Some("login")));
        assert_eq!(window.classes, ["panel", "dark"]);
        assert_eq!(window.children[0].action.as_deref(), Some("login"));

        assert!(matches!(parse_markup("<ui><div/></ui>"), Err(UiError::UnknownTag(tag)) if tag == "div"));
        assert!(matches!(parse_markup(r#"<ui><row onclick="x"/></ui>"#), Err(UiError::UnknownAttribute { .. })));
        assert!(parse_markup("<ui><label><row/></label></ui>").is_err());
        assert!(parse_markup("<ui>hello</ui>").is_err());
        assert!(parse_markup("<window/>").is_err());
        let ui =
            parse_markup(r#"<ui><input bind="login.password" type="password" placeholder="Password"/></ui>"#).unwrap();
        assert!(ui.children[0].password && ui.children[0].placeholder.as_deref() == Some("Password"));
        assert!(parse_markup(r#"<ui><input type="date"/></ui>"#).is_err());
    }
}

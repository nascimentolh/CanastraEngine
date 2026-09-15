//! `<select>`: a row showing the chosen value that opens its options below it, drawn over everything else.
//!
//! The host keeps which select is open: clicking the row reports `ui.open:<bind>`, and the host binds that
//! bind key under `ui.open` while the select stays open.

use crate::markup::{Element, Tag};

/// Prefix of the action a select's row reports; the rest is the select's bind key.
pub const OPEN_ACTION: &str = "ui.open:";
/// Data key holding the bind key of the open select.
pub const OPEN_KEY: &str = "ui.open";

/// The elements `select` stands for: a `select-box` column holding the `select` row (its `select-value` and
/// `select-arrow` labels) and, when `open`, the `select-options` overlay of `option` buttons.
pub(crate) fn expand(select: &Element, open: bool) -> Element {
    let bind = select.bind.clone().unwrap_or_default();
    let with_class = |tag, class: &str| Element { classes: vec![class.to_owned()], ..Element::new(tag) };
    let row = Element {
        classes: [vec!["select".to_owned()], select.classes.clone()].concat(),
        action: Some(format!("{OPEN_ACTION}{bind}")),
        children: vec![
            Element { bind: Some(bind), ..with_class(Tag::Label, "select-value") },
            Element { text: Some("\u{25BC}".to_owned()), ..with_class(Tag::Label, "select-arrow") },
        ],
        ..Element::new(Tag::Row)
    };
    let mut children = vec![row];
    if open {
        children.push(Element {
            repeat: select.options.clone(),
            overlay: true,
            children: vec![Element {
                bind: Some("item.name".to_owned()),
                action: select.action.clone(),
                ..with_class(Tag::Button, "option")
            }],
            ..with_class(Tag::Column, "select-options")
        });
    }
    Element { id: select.id.clone(), children, ..with_class(Tag::Column, "select-box") }
}

//! One UI screen: its markup and stylesheet, the laid-out frame and pointer state.

use std::path::{Path, PathBuf};

use canastra_ui::{Element, Frame, StyleSheet, TextMeasure};

const MARKUP: &str = "login.ui";
const STYLESHEET: &str = "theme.css";

pub(crate) struct Screen {
    folder: PathBuf,
    ui: Element,
    sheet: StyleSheet,
    pub(crate) frame: Frame,
    hovered: Option<usize>,
    /// Last pointer position in logical pixels.
    pointer: (f32, f32),
    pub(crate) status: String,
}

impl Screen {
    pub(crate) fn load(folder: PathBuf) -> Result<Self, String> {
        let (ui, sheet) = read(&folder)?;
        Ok(Self {
            folder,
            ui,
            sheet,
            frame: Frame::default(),
            hovered: None,
            pointer: (0.0, 0.0),
            status: String::new(),
        })
    }

    /// Re-reads markup and stylesheet; on failure the old ones stay and the status shows why.
    pub(crate) fn reload(&mut self) {
        match read(&self.folder) {
            Ok((ui, sheet)) => {
                (self.ui, self.sheet) = (ui, sheet);
                self.status = "UI reloaded".into();
            }
            Err(error) => self.status = error,
        }
    }

    /// Lays the screen out for a `viewport` in logical pixels.
    pub(crate) fn layout(&mut self, viewport: [f32; 2], text: &mut dyn TextMeasure) {
        let status = &self.status;
        let bindings = |key: &str| match key {
            "app.version" => Some(concat!("v", env!("CARGO_PKG_VERSION")).to_owned()),
            "app.status" => Some(status.clone()),
            _ => None,
        };
        match canastra_ui::build(&self.ui, &self.sheet, viewport, self.hovered, text, &bindings) {
            Ok(frame) => self.frame = frame,
            Err(error) => eprintln!("layout: {error}"),
        }
    }

    /// Moves the pointer; true when the hovered element changed.
    pub(crate) fn pointer(&mut self, x: f32, y: f32) -> bool {
        self.pointer = (x, y);
        let hovered = self.frame.hit(x, y).map(|hit| hit.element);
        std::mem::replace(&mut self.hovered, hovered) != hovered
    }

    /// The action of the element under the pointer, if any.
    pub(crate) fn click(&self) -> Option<&str> {
        self.frame.hit(self.pointer.0, self.pointer.1)?.action.as_deref()
    }
}

fn read(folder: &Path) -> Result<(Element, StyleSheet), String> {
    let read = |name: &str| {
        let path = folder.join(name);
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let ui = canastra_ui::parse_markup(&read(MARKUP)?).map_err(|error| format!("{MARKUP}: {error}"))?;
    let sheet = canastra_ui::parse_stylesheet(&read(STYLESHEET)?).map_err(|error| format!("{STYLESHEET}: {error}"))?;
    Ok((ui, sheet))
}

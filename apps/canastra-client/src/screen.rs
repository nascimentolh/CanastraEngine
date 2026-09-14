//! One UI screen: its markup and stylesheet, the laid-out frame, pointer and input focus.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use canastra_ui::{Element, Frame, Hit, StyleSheet, TextMeasure, Transitions, UiState};

const MARKUP: &str = "login.ui";
const STYLESHEET: &str = "theme.css";
/// How long the caret stays on, then off.
const BLINK: Duration = Duration::from_millis(530);

pub(crate) struct Screen {
    folder: PathBuf,
    ui: Element,
    sheet: StyleSheet,
    pub(crate) frame: Frame,
    state: UiState,
    /// Last pointer position in logical pixels.
    pointer: (f32, f32),
    pub(crate) status: String,
    /// What was typed into each input, by its data key.
    values: HashMap<String, String>,
    /// When the caret last turned on; typing restarts the blink so the caret stays visible.
    caret_since: Instant,
    transitions: Transitions,
    /// Zero of the clock transitions run on.
    opened: Instant,
}

impl Screen {
    pub(crate) fn load(folder: PathBuf) -> Result<Self, String> {
        let (ui, sheet) = read(&folder)?;
        Ok(Self {
            folder,
            ui,
            sheet,
            frame: Frame::default(),
            state: UiState::default(),
            pointer: (0.0, 0.0),
            status: String::new(),
            values: HashMap::new(),
            caret_since: Instant::now(),
            transitions: Transitions::default(),
            opened: Instant::now(),
        })
    }

    /// Re-reads markup and stylesheet; on failure the old ones stay and the status shows why.
    pub(crate) fn reload(&mut self) {
        match read(&self.folder) {
            Ok((ui, sheet)) => {
                (self.ui, self.sheet, self.transitions) = (ui, sheet, Transitions::default());
                self.status = "UI reloaded".into();
            }
            Err(error) => self.status = error,
        }
    }

    /// Lays the screen out for a `viewport` in logical pixels.
    pub(crate) fn layout(&mut self, viewport: [f32; 2], text: &mut dyn TextMeasure) {
        self.state.caret = self.blinks().is_multiple_of(2);
        self.state.time = self.opened.elapsed().as_secs_f32();
        let (status, values) = (&self.status, &self.values);
        let bindings = |key: &str| match key {
            "app.version" => Some(concat!("v", env!("CARGO_PKG_VERSION")).to_owned()),
            "app.status" => Some(status.clone()),
            _ => values.get(key).cloned(),
        };
        match canastra_ui::build(&self.ui, &self.sheet, viewport, self.state, &mut self.transitions, text, &bindings) {
            Ok(frame) => self.frame = frame,
            Err(error) => eprintln!("layout: {error}"),
        }
    }

    /// Moves the pointer; true when the hovered element changed.
    pub(crate) fn pointer(&mut self, x: f32, y: f32) -> bool {
        self.pointer = (x, y);
        let hovered = self.frame.hit(x, y).map(|hit| hit.element);
        std::mem::replace(&mut self.state.hovered, hovered) != hovered
    }

    /// Focuses the input under the pointer, or returns the action of the element there.
    pub(crate) fn click(&mut self) -> Option<String> {
        let hit = self.frame.hit(self.pointer.0, self.pointer.1).cloned();
        let field = hit.as_ref().filter(|hit| hit.field.is_some());
        self.focus(field.map(|hit| hit.element));
        hit.filter(|hit| hit.field.is_none())?.action
    }

    /// Moves focus to the next input, or the previous one, wrapping around.
    pub(crate) fn focus_next(&mut self, backwards: bool) {
        let fields: Vec<usize> = self.frame.fields().map(|hit| hit.element).collect();
        let current = fields.iter().position(|&element| Some(element) == self.state.focused);
        let next = match (current, backwards) {
            (None, false) => 0,
            (None, true) => fields.len().saturating_sub(1),
            (Some(index), false) => (index + 1) % fields.len(),
            (Some(index), true) => (index + fields.len() - 1) % fields.len(),
        };
        self.focus(fields.get(next).copied());
    }

    pub(crate) fn type_text(&mut self, text: &str) {
        if let Some(key) = self.focused().and_then(|hit| hit.field.clone()) {
            self.values.entry(key).or_default().extend(text.chars().filter(|c| !c.is_control()));
            self.caret_since = Instant::now();
        }
    }

    pub(crate) fn backspace(&mut self) {
        if let Some(value) = self.focused().and_then(|hit| hit.field.clone()).and_then(|key| self.values.get_mut(&key))
        {
            value.pop();
            self.caret_since = Instant::now();
        }
    }

    /// The action of the focused input, run when Enter is pressed.
    pub(crate) fn submit(&self) -> Option<String> {
        self.focused()?.action.clone()
    }

    /// When the caret next turns on or off, while an input has focus.
    pub(crate) fn next_blink(&self) -> Option<Instant> {
        self.state.focused?;
        Some(self.caret_since + BLINK * self.blinks().saturating_add(1))
    }

    fn focused(&self) -> Option<&Hit> {
        self.frame.fields().find(|hit| Some(hit.element) == self.state.focused)
    }

    fn focus(&mut self, element: Option<usize>) {
        self.state.focused = element;
        self.caret_since = Instant::now();
    }

    /// Whole blink periods since the caret last turned on.
    fn blinks(&self) -> u32 {
        u32::try_from(self.caret_since.elapsed().as_millis() / BLINK.as_millis()).unwrap_or(u32::MAX)
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

//! Studio window: entity list, editor, validation and saving.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use canastra_data::id::{ItemId, NpcId, SkillId};
use canastra_data::item::Item;
use canastra_data::npc::Npc;
use canastra_data::skill::Skill;
use canastra_data::text::Locale;
use canastra_data::{GameData, Issue, format};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, Ui};

use crate::forms;
use crate::icons::Icons;

/// Edits to the same entity within this window collapse into one undo step.
const UNDO_MERGE: Duration = Duration::from_millis(800);
const ROW_HEIGHT: f32 = 22.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Items,
    Skills,
    Npcs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    Item(ItemId),
    Skill(SkillId),
    Npc(NpcId),
}

/// The state of one entity before an edit.
#[derive(Clone, PartialEq)]
enum Snapshot {
    Item(Item),
    Skill(Skill),
    Npc(Npc),
}

impl Snapshot {
    fn selection(&self) -> Selection {
        match self {
            Self::Item(item) => Selection::Item(item.id),
            Self::Skill(skill) => Selection::Skill(skill.id),
            Self::Npc(npc) => Selection::Npc(npc.id),
        }
    }
}

pub(crate) struct Studio {
    path: PathBuf,
    data: GameData,
    icons: Icons,
    tab: Tab,
    search: String,
    selected: Option<Selection>,
    skill_level: u32,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_edit: Option<(Selection, Instant)>,
    issues: Vec<Issue>,
    dirty: bool,
    status: String,
}

impl Studio {
    pub(crate) fn new(path: PathBuf, data: GameData, icons: Icons) -> Self {
        let issues = data.validate();
        let status = format!("Opened {}", path.display());
        Self {
            path,
            data,
            icons,
            tab: Tab::Items,
            search: String::new(),
            selected: None,
            skill_level: 1,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            issues,
            dirty: false,
            status,
        }
    }

    fn snapshot(&self, key: Selection) -> Option<Snapshot> {
        Some(match key {
            Selection::Item(id) => Snapshot::Item(self.data.items.get(&id)?.clone()),
            Selection::Skill(id) => Snapshot::Skill(self.data.skills.get(&id)?.clone()),
            Selection::Npc(id) => Snapshot::Npc(self.data.npcs.get(&id)?.clone()),
        })
    }

    /// Puts `snapshot` back and returns what it replaced.
    fn restore(&mut self, snapshot: Snapshot) -> Option<Snapshot> {
        let selection = snapshot.selection();
        let previous = self.snapshot(selection);
        match snapshot {
            Snapshot::Item(item) => self.data.items.insert(item.id, item).map(drop),
            Snapshot::Skill(skill) => self.data.skills.insert(skill.id, skill).map(drop),
            Snapshot::Npc(npc) => self.data.npcs.insert(npc.id, npc).map(drop),
        };
        self.selected = Some(selection);
        self.changed();
        previous
    }

    fn step(&mut self, back: bool) {
        let from = if back { &mut self.undo } else { &mut self.redo };
        let Some(snapshot) = from.pop() else { return };
        if let Some(previous) = self.restore(snapshot) {
            let to = if back { &mut self.redo } else { &mut self.undo };
            to.push(previous);
        }
        self.last_edit = None;
    }

    fn changed(&mut self) {
        self.dirty = true;
        self.issues = self.data.validate();
    }

    fn save(&mut self) {
        if !self.issues.is_empty() {
            self.status = format!("Not saved: fix {} issue(s) first", self.issues.len());
            return;
        }
        self.status = match write_safely(&self.path, &format::encode(&self.data)) {
            Ok(()) => {
                self.dirty = false;
                format!("Saved {} (previous version kept as .bak)", self.path.display())
            }
            Err(error) => format!("Not saved: {error}"),
        };
    }

    fn rows(&self) -> Vec<(Selection, String)> {
        let needle = self.search.to_lowercase();
        let matches = |id: u32, name: &str| {
            needle.is_empty() || id.to_string() == needle || name.to_lowercase().contains(&needle)
        };
        match self.tab {
            Tab::Items => self
                .data
                .items
                .values()
                .filter(|item| matches(item.id.0, item.name.get(Locale::En).unwrap_or_default()))
                .map(|item| (Selection::Item(item.id), label(item.id.0, item.name.get(Locale::En))))
                .collect(),
            Tab::Skills => self
                .data
                .skills
                .values()
                .map(|skill| (skill, skill.levels.values().next().and_then(|level| level.name.get(Locale::En))))
                .filter(|(skill, name)| matches(skill.id.0, name.unwrap_or_default()))
                .map(|(skill, name)| (Selection::Skill(skill.id), label(skill.id.0, name)))
                .collect(),
            Tab::Npcs => self
                .data
                .npcs
                .values()
                .filter(|npc| matches(npc.id.0, npc.name.get(Locale::En).unwrap_or_default()))
                .map(|npc| (Selection::Npc(npc.id), label(npc.id.0, npc.name.get(Locale::En))))
                .collect(),
        }
    }

    fn list(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (tab, name) in [(Tab::Items, "Items"), (Tab::Skills, "Skills"), (Tab::Npcs, "NPCs")] {
                ui.selectable_value(&mut self.tab, tab, name);
            }
        });
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search by id or name"));
        ui.separator();
        let rows = self.rows();
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
            for (key, text) in rows.get(range).unwrap_or_default() {
                if ui.selectable_label(self.selected == Some(*key), text).clicked() {
                    self.selected = Some(*key);
                    if let Selection::Skill(id) = key {
                        self.skill_level =
                            self.data.skills.get(id).and_then(|s| s.levels.keys().next().copied()).unwrap_or(1);
                    }
                }
            }
        });
    }

    fn editor(&mut self, ui: &mut Ui) {
        let Some(key) = self.selected else {
            ui.centered_and_justified(|ui| ui.label("Select an entry on the left."));
            return;
        };
        let before = self.snapshot(key);
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match key {
            Selection::Item(id) => {
                if let Some(item) = self.data.items.get_mut(&id) {
                    forms::item(ui, item, &mut self.icons);
                }
            }
            Selection::Skill(id) => {
                if let Some(skill) = self.data.skills.get_mut(&id) {
                    forms::skill(ui, skill, &mut self.skill_level, &mut self.icons);
                }
            }
            Selection::Npc(id) => {
                if let Some(npc) = self.data.npcs.get_mut(&id) {
                    forms::npc(ui, npc);
                }
            }
        });
        let Some(before) = before else { return };
        if self.snapshot(key).as_ref() != Some(&before) {
            let merge = self.last_edit.is_some_and(|(last, at)| last == key && at.elapsed() < UNDO_MERGE);
            if !merge {
                self.undo.push(before);
            }
            self.redo.clear();
            self.last_edit = Some((key, Instant::now()));
            self.changed();
        }
    }
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let command = |key| KeyboardShortcut::new(Modifiers::COMMAND, key);
        if ui.input_mut(|input| input.consume_shortcut(&command(Key::S))) {
            self.save();
        }
        if ui.input_mut(|input| input.consume_shortcut(&command(Key::Z))) {
            self.step(true);
        }
        if ui.input_mut(|input| input.consume_shortcut(&command(Key::Y))) {
            self.step(false);
        }

        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                let title = if self.dirty { "Canastra Studio *" } else { "Canastra Studio" };
                ui.strong(title);
                ui.separator();
                if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo")).clicked() {
                    self.step(true);
                }
                if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo")).clicked() {
                    self.step(false);
                }
                if ui.add_enabled(self.issues.is_empty(), egui::Button::new("Save")).clicked() {
                    self.save();
                }
                ui.separator();
                ui.label(&self.status);
            });
        });
        egui::Panel::bottom("issues").resizable(true).default_size(120.0).show(ui, |ui| {
            ui.strong(format!("{} issue(s) block saving", self.issues.len()));
            egui::ScrollArea::vertical().show(ui, |ui| {
                for issue in &self.issues {
                    ui.label(format!("{:?}: {:?}", issue.subject, issue.problem));
                }
            });
        });
        egui::Panel::left("list").resizable(true).default_size(320.0).show(ui, |ui| self.list(ui));
        egui::CentralPanel::default().show(ui, |ui| self.editor(ui));
    }
}

fn label(id: u32, name: Option<&str>) -> String {
    format!("{id}  {}", name.unwrap_or_default())
}

/// Writes next to the target first, keeps the previous file as `.bak`, then swaps the new one in,
/// so a failure at any point never leaves a half-written data file.
fn write_safely(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let temp = path.with_extension("cana.tmp");
    fs::write(&temp, bytes)?;
    // Reading the written file back proves the bytes on disk decode before anything is replaced.
    if format::decode(&fs::read(&temp)?).is_err() {
        return Err(std::io::Error::other("written data does not read back"));
    }
    if path.exists() {
        fs::copy(path, path.with_extension("cana.bak"))?;
    }
    fs::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_keeps_the_previous_file() {
        let dir = std::env::temp_dir().join(format!("canastra-studio-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.cana");
        let first = GameData::default();
        let mut second = GameData::default();
        second.items.insert(ItemId(1), Item::new(ItemId(1)));

        write_safely(&path, &format::encode(&first)).unwrap();
        write_safely(&path, &format::encode(&second)).unwrap();

        assert_eq!(format::decode(&fs::read(&path).unwrap()).unwrap(), second);
        assert_eq!(format::decode(&fs::read(path.with_extension("cana.bak")).unwrap()).unwrap(), first);
        assert!(!path.with_extension("cana.tmp").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}

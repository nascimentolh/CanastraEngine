//! Editors for each kind of game data. Every widget writes straight into the domain types,
//! so an edit that the types cannot represent is impossible to make. The widgets they share live here.

mod class;
mod creature;
mod item;

use std::fmt::Debug;

use canastra_data::asset::AssetRef;
use canastra_data::id::{SkillId, SkillRef};
use canastra_data::text::{Locale, Localized};
use eframe::egui::{self, Color32, DragValue, Grid, Ui};

pub(crate) use class::class;
pub(crate) use creature::{npc, skill};
pub(crate) use item::item;

fn skills(ui: &mut Ui, id: &str, skills: &mut Vec<SkillRef>) {
    let mut remove = None;
    Grid::new(id).num_columns(3).show(ui, |ui| {
        for (index, skill) in skills.iter_mut().enumerate() {
            let SkillId(skill_id) = &mut skill.id;
            ui.add(DragValue::new(skill_id).prefix("id "));
            ui.add(DragValue::new(&mut skill.level).prefix("level "));
            if ui.small_button("Remove").clicked() {
                remove = Some(index);
            }
            ui.end_row();
        }
    });
    if let Some(index) = remove {
        skills.remove(index);
    }
    if ui.button("Add skill").clicked() {
        skills.push(SkillRef { id: SkillId(0), level: 1 });
    }
}

/// English text; more than one row makes it multi-line.
fn text(ui: &mut Ui, label: &str, value: &mut Localized, rows: usize) {
    ui.label(label);
    let mut current = value.get(Locale::En).unwrap_or_default().to_owned();
    let edit = if rows > 1 {
        egui::TextEdit::multiline(&mut current).desired_rows(rows)
    } else {
        egui::TextEdit::singleline(&mut current)
    };
    if ui.add(edit).changed() {
        value.set(Locale::En, current);
    }
    ui.end_row();
}

fn plain(ui: &mut Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.text_edit_singleline(value);
    ui.end_row();
}

fn number<T: egui::emath::Numeric>(ui: &mut Ui, label: &str, value: &mut T) {
    ui.label(label);
    ui.add(DragValue::new(value));
    ui.end_row();
}

fn optional(ui: &mut Ui, label: &str, value: &mut Option<u32>) {
    ui.label(label);
    toggled(ui, value);
    ui.end_row();
}

/// A checkbox that turns the value on or off, and the value while on.
fn toggled(ui: &mut Ui, value: &mut Option<u32>) {
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, "").changed() {
            *value = enabled.then_some(0);
        }
        if let Some(inner) = value {
            ui.add(DragValue::new(inner));
        }
    });
}

fn choice<T: Copy + PartialEq + Debug>(ui: &mut Ui, label: &str, value: &mut T, all: &[T]) {
    ui.label(label);
    combo(ui, label, value, all);
    ui.end_row();
}

fn combo<T: Copy + PartialEq + Debug>(ui: &mut Ui, id: impl std::hash::Hash + Debug, value: &mut T, all: &[T]) {
    egui::ComboBox::from_id_salt(id).selected_text(format!("{value:?}")).show_ui(ui, |ui| {
        for &option in all {
            ui.selectable_value(value, option, format!("{option:?}"));
        }
    });
}

/// Edits a path as text; only a valid `Package.Object` path is written back, and an invalid
/// one stays in the field in red until it is fixed.
fn asset<K>(ui: &mut Ui, label: &str, value: &mut Option<AssetRef<K>>) {
    ui.label(label);
    let id = ui.make_persistent_id(("asset", label));
    let stored = value.as_ref().map(|asset| asset.path().to_owned()).unwrap_or_default();
    let mut draft = ui.data_mut(|data| data.get_temp::<String>(id)).unwrap_or_else(|| stored.clone());
    let invalid = !draft.is_empty() && AssetRef::<K>::parse(&draft).is_err();
    let edit = egui::TextEdit::singleline(&mut draft).text_color_opt(invalid.then_some(Color32::LIGHT_RED));
    let response = ui.add(edit);
    if response.changed() {
        if draft.is_empty() {
            *value = None;
        } else if let Ok(parsed) = AssetRef::parse(&draft) {
            *value = Some(parsed);
        }
    }
    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, draft));
    } else {
        ui.data_mut(|data| data.remove::<String>(id));
    }
    ui.end_row();
}

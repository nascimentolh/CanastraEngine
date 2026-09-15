//! The class editor. A starting class's kit belongs to the server owner: the migrated items are only a
//! suggestion to change here.

use std::collections::BTreeMap;

use canastra_data::class::{InitialItem, Origin, PlayerClass};
use canastra_data::id::{ClassId, ItemId};
use canastra_data::item::Item;
use canastra_data::text::Locale;
use eframe::egui::{DragValue, Grid, Ui};

use super::{text, toggled};
use crate::icons::Icons;

pub(crate) fn class(ui: &mut Ui, class: &mut PlayerClass, items: &BTreeMap<ItemId, Item>, icons: &mut Icons) {
    ui.heading(format!("Class {}", class.id.0));
    Grid::new("class").num_columns(2).striped(true).show(ui, |ui| {
        text(ui, "Name", &mut class.name, 1);
    });
    match &mut class.origin {
        Origin::Advanced { parent: ClassId(parent) } => {
            ui.label(format!("Advanced from class {parent}; its kit comes from the line's starting class."));
        }
        Origin::Starting(start) => {
            ui.label(format!("Starting class: {:?} {:?}", start.race, start.archetype));
            ui.separator();
            ui.strong("Initial items");
            ui.label("Given in this order to every new character. Worn items are equipped, and timed ones disappear that many minutes after creation.");
            initial_items(ui, &mut start.initial_items, items, icons);
        }
    }
}

fn initial_items(ui: &mut Ui, list: &mut Vec<InitialItem>, items: &BTreeMap<ItemId, Item>, icons: &mut Icons) {
    let mut remove = None;
    Grid::new("initial-items").num_columns(7).striped(true).show(ui, |ui| {
        for heading in ["", "Item", "Name", "Count", "Worn", "Lasts (minutes)", ""] {
            ui.strong(heading);
        }
        ui.end_row();
        for (index, initial) in list.iter_mut().enumerate() {
            let item = items.get(&initial.item);
            icons.show(ui, item.and_then(|item| item.visual.icon.as_ref()), 20.0);
            let ItemId(id) = &mut initial.item;
            ui.add(DragValue::new(id));
            ui.label(item.map_or("unknown item", |item| item.name.get(Locale::En).unwrap_or_default()));
            ui.add(DragValue::new(&mut initial.count).range(1..=u64::MAX));
            ui.checkbox(&mut initial.equipped, "");
            toggled(ui, &mut initial.lasts_minutes);
            if ui.small_button("Remove").clicked() {
                remove = Some(index);
            }
            ui.end_row();
        }
    });
    if let Some(index) = remove {
        list.remove(index);
    }
    if ui.button("Add item").clicked() {
        list.push(InitialItem { item: ItemId(0), count: 1, equipped: false, lasts_minutes: None });
    }
}

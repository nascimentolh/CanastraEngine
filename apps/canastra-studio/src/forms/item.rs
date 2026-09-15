//! The item editor.

use canastra_data::item::{
    Armor, ArmorType, EquipSlot, EtcItem, EtcItemType, Grade, HandlerKey, Item, ItemAction, ItemKind, Material, Stat,
    StatModifier, StatOp, Weapon, WeaponType,
};
use eframe::egui::{DragValue, Grid, Ui};

use super::{asset, choice, combo, number, optional, skills, text};
use crate::icons::Icons;

pub(crate) fn item(ui: &mut Ui, item: &mut Item, icons: &mut Icons) {
    ui.horizontal(|ui| {
        icons.show(ui, item.visual.icon.as_ref(), 32.0);
        ui.heading(format!("Item {}", item.id.0));
    });
    Grid::new("item").num_columns(2).striped(true).show(ui, |ui| {
        text(ui, "Name", &mut item.name, 1);
        text(ui, "Additional name", &mut item.additional_name, 1);
        text(ui, "Description", &mut item.description, 3);
        asset(ui, "Icon", &mut item.visual.icon);
        number(ui, "Weight", &mut item.weight);
        number(ui, "Price", &mut item.price);
        number(ui, "Crystal count", &mut item.crystal_count);
        choice(ui, "Material", &mut item.material, Material::ALL);
        choice(ui, "Grade", &mut item.grade, Grade::ALL);
        choice(ui, "Action", &mut item.action, ItemAction::ALL);
        handler(ui, &mut item.handler);
        number(ui, "Reuse delay (ms)", &mut item.reuse_delay_ms);
        optional(ui, "Shared reuse group", &mut item.shared_reuse_group);
        optional(ui, "Mana (minutes)", &mut item.mana_minutes);
        optional(ui, "Lifetime (minutes)", &mut item.lifetime_minutes);
        kind(ui, &mut item.kind);
    });

    ui.collapsing("Rules", |ui| {
        let flags = &mut item.flags;
        ui.horizontal_wrapped(|ui| {
            for (label, flag) in [
                ("Stackable", &mut flags.stackable),
                ("Sellable", &mut flags.sellable),
                ("Droppable", &mut flags.droppable),
                ("Destroyable", &mut flags.destroyable),
                ("Tradable", &mut flags.tradable),
                ("Depositable", &mut flags.depositable),
                ("Freightable", &mut flags.freightable),
                ("Enchantable", &mut flags.enchantable),
                ("Elementable", &mut flags.elementable),
                ("Quest", &mut flags.quest),
                ("Olympiad restricted", &mut flags.olympiad_restricted),
                ("For NPC", &mut flags.for_npc),
                ("Immediate effect", &mut flags.immediate_effect),
                ("Self resurrection", &mut flags.self_resurrection),
            ] {
                ui.checkbox(flag, label);
            }
        });
    });
    ui.collapsing("Skills", |ui| skills(ui, "item-skills", &mut item.skills));
    ui.collapsing("Stats", |ui| stats(ui, &mut item.stats));
}

fn kind(ui: &mut Ui, kind: &mut ItemKind) {
    ui.label("Kind");
    ui.horizontal(|ui| {
        let current = match kind {
            ItemKind::Weapon(_) => 0,
            ItemKind::Armor(_) => 1,
            ItemKind::Etc(_) => 2,
        };
        for (index, label) in ["Weapon", "Armor", "Etc"].into_iter().enumerate() {
            if ui.selectable_label(current == index, label).clicked() && current != index {
                *kind = match index {
                    0 => ItemKind::Weapon(Weapon::default()),
                    1 => ItemKind::Armor(Armor::default()),
                    _ => ItemKind::Etc(EtcItem::default()),
                };
            }
        }
    });
    ui.end_row();
    match kind {
        ItemKind::Weapon(weapon) => {
            choice(ui, "Weapon type", &mut weapon.weapon_type, WeaponType::ALL);
            choice(ui, "Slot", &mut weapon.slot, EquipSlot::ALL);
            number(ui, "Soulshots", &mut weapon.soulshots);
            number(ui, "Spiritshots", &mut weapon.spiritshots);
            number(ui, "Random damage", &mut weapon.random_damage);
            number(ui, "Attack range", &mut weapon.attack_range);
            number(ui, "MP consume", &mut weapon.mp_consume);
            ui.label("Magic weapon");
            ui.checkbox(&mut weapon.magic, "");
            ui.end_row();
        }
        ItemKind::Armor(armor) => {
            choice(ui, "Armor type", &mut armor.armor_type, ArmorType::ALL);
            choice(ui, "Slot", &mut armor.slot, EquipSlot::ALL);
        }
        ItemKind::Etc(etc) => {
            choice(ui, "Etc type", &mut etc.etc_type, EtcItemType::ALL);
            choice(ui, "Slot", &mut etc.slot, EquipSlot::ALL);
        }
    }
}

fn stats(ui: &mut Ui, stats: &mut Vec<StatModifier>) {
    let mut remove = None;
    Grid::new("stats").num_columns(4).show(ui, |ui| {
        for (index, modifier) in stats.iter_mut().enumerate() {
            combo(ui, ("stat", index), &mut modifier.stat, Stat::ALL);
            combo(ui, ("op", index), &mut modifier.op, StatOp::ALL);
            ui.add(DragValue::new(&mut modifier.value).speed(0.1));
            if ui.small_button("Remove").clicked() {
                remove = Some(index);
            }
            ui.end_row();
        }
    });
    if let Some(index) = remove {
        stats.remove(index);
    }
    if ui.button("Add stat").clicked() {
        stats.push(StatModifier { stat: Stat::PAtk, op: StatOp::Add, value: 0.0, order: None });
    }
}

fn handler(ui: &mut Ui, value: &mut Option<HandlerKey>) {
    ui.label("Handler");
    let mut current = value.as_ref().map(|HandlerKey(key)| key.clone()).unwrap_or_default();
    if ui.text_edit_singleline(&mut current).changed() {
        *value = (!current.trim().is_empty()).then(|| HandlerKey(current.trim().to_owned()));
    }
    ui.end_row();
}

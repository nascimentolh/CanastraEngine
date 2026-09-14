//! Editors for each kind of game data. Every widget writes straight into the domain types,
//! so an edit that the types cannot represent is impossible to make.

use std::fmt::Debug;

use canastra_data::asset::AssetRef;
use canastra_data::id::{SkillId, SkillRef};
use canastra_data::item::{
    Armor, ArmorType, EquipSlot, EtcItem, EtcItemType, Grade, HandlerKey, Item, ItemAction, ItemKind, Material, Stat,
    StatModifier, StatOp, Weapon, WeaponType,
};
use canastra_data::npc::{Npc, NpcType, Race, Sex};
use canastra_data::skill::{Skill, SkillOperate};
use canastra_data::text::{Locale, Localized};
use eframe::egui::{self, Color32, DragValue, Grid, Ui};

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

pub(crate) fn skill(ui: &mut Ui, skill: &mut Skill, level: &mut u32, icons: &mut Icons) {
    ui.heading(format!("Skill {}", skill.id.0));
    Grid::new("skill").num_columns(2).show(ui, |ui| {
        choice(ui, "Operate", &mut skill.operate, SkillOperate::ALL);
        ui.label("Level");
        egui::ComboBox::from_id_salt("skill-level").selected_text(level.to_string()).show_ui(ui, |ui| {
            for &number in skill.levels.keys() {
                ui.selectable_value(level, number, number.to_string());
            }
        });
        ui.end_row();
    });
    let Some(entry) = skill.levels.get_mut(level) else {
        return;
    };
    ui.horizontal(|ui| {
        icons.show(ui, entry.icon.as_ref(), 32.0);
        ui.heading(entry.name.get(Locale::En).unwrap_or_default());
    });
    Grid::new("skill-level-fields").num_columns(2).striped(true).show(ui, |ui| {
        text(ui, "Name", &mut entry.name, 1);
        text(ui, "Description", &mut entry.description, 3);
        text(ui, "Enchant name", &mut entry.enchant_name, 1);
        text(ui, "Enchant description", &mut entry.enchant_description, 3);
        asset(ui, "Icon", &mut entry.icon);
        asset(ui, "Icon panel", &mut entry.icon_panel);
        plain(ui, "Enchant icon code", &mut entry.enchant_icon);
        plain(ui, "Animation code", &mut entry.animation);
        plain(ui, "Effect code", &mut entry.effect);
    });
}

pub(crate) fn npc(ui: &mut Ui, npc: &mut Npc) {
    ui.heading(format!("NPC {}", npc.id.0));
    Grid::new("npc").num_columns(2).striped(true).show(ui, |ui| {
        text(ui, "Name", &mut npc.name, 1);
        text(ui, "Title", &mut npc.title, 1);
        ui.label("Title color");
        ui.add(DragValue::new(&mut npc.title_color).hexadecimal(8, false, true));
        ui.end_row();
        let NpcType(npc_type) = &mut npc.npc_type;
        plain(ui, "Type", npc_type);
        number(ui, "Level", &mut npc.level);
        choice(ui, "Race", &mut npc.race, Race::ALL);
        choice(ui, "Sex", &mut npc.sex, Sex::ALL);
        number(ui, "HP", &mut npc.hp);
        number(ui, "MP", &mut npc.mp);
        number(ui, "Walk speed", &mut npc.walk_speed);
        number(ui, "Run speed", &mut npc.run_speed);
        number(ui, "Collision radius", &mut npc.collision.radius);
        number(ui, "Collision height", &mut npc.collision.height);
    });
    ui.collapsing("Rules", |ui| {
        let flags = &mut npc.flags;
        ui.horizontal_wrapped(|ui| {
            for (label, flag) in [
                ("Attackable", &mut flags.attackable),
                ("Targetable", &mut flags.targetable),
                ("Talkable", &mut flags.talkable),
                ("Show name", &mut flags.show_name),
                ("Undying", &mut flags.undying),
                ("Flying", &mut flags.flying),
                ("Can move", &mut flags.can_move),
            ] {
                ui.checkbox(flag, label);
            }
        });
    });
    ui.collapsing("Skills", |ui| skills(ui, "npc-skills", &mut npc.skills));
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
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, "").changed() {
            *value = enabled.then_some(0);
        }
        if let Some(inner) = value {
            ui.add(DragValue::new(inner));
        }
    });
    ui.end_row();
}

fn handler(ui: &mut Ui, value: &mut Option<HandlerKey>) {
    ui.label("Handler");
    let mut current = value.as_ref().map(|HandlerKey(key)| key.clone()).unwrap_or_default();
    if ui.text_edit_singleline(&mut current).changed() {
        *value = (!current.trim().is_empty()).then(|| HandlerKey(current.trim().to_owned()));
    }
    ui.end_row();
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

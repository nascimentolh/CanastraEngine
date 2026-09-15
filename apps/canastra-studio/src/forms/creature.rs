//! The skill and NPC editors.

use canastra_data::npc::{Npc, NpcType, Race, Sex};
use canastra_data::skill::{Skill, SkillOperate};
use canastra_data::text::Locale;
use eframe::egui::{self, DragValue, Grid, Ui};

use super::{asset, choice, number, plain, skills, text};
use crate::icons::Icons;

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

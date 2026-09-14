//! Skills: existence, levels and operate type from the server; names, icons and sounds from the client.

use std::collections::{BTreeMap, BTreeSet};

use canastra_data::id::SkillId;
use canastra_data::item::Body;
use canastra_data::skill::{Skill, SkillLevel, SkillOperate, SkillSounds, SoundCue};
use canastra_data::text::Localized;
use l2_dat::Value;
use roxmltree::Node;

use crate::fields::{Assets, Fields, Result, code, localized, parse};
use crate::{Report, Severity, Sources, index, server_index};

/// Client copies of skill mechanics; they return with the server's skill model.
const GAMEPLAY_COPIES: &[&str] =
    &["operate_type", "mp_consume", "hp_consume", "cast_range", "hit_time", "is_magic", "debuff"];

/// Values whose meaning is not known; nothing in the engine reads them yet.
const UNKNOWN: &[&str] = &[
    "icon_type",
    "cast_style",
    "enchanted",
    "enchant_skill_level",
    "rumble_self",
    "rumble_target",
    "GaugeTime",
    "AdditionalTag",
];

/// Enchant route `r` owns levels `100 * r + 1 ..= 100 * r + 30`.
const ENCHANT_ROUTE_SPAN: u32 = 100;
const ENCHANT_ROUTE_LEVELS: u32 = 30;

type LevelKey = (SkillId, u32);

struct ServerSkill {
    name: String,
    levels: u32,
    routes: u32,
    operate: SkillOperate,
}

impl ServerSkill {
    fn defines(&self, level: u32) -> bool {
        match (level / ENCHANT_ROUTE_SPAN, level % ENCHANT_ROUTE_SPAN) {
            (0, _) => (1..=self.levels).contains(&level),
            (route, step) => route <= self.routes && (1..=ENCHANT_ROUTE_LEVELS).contains(&step),
        }
    }

    fn all_levels(&self) -> impl Iterator<Item = u32> {
        let enchants = (1..=self.routes).flat_map(|route| {
            let base = route * ENCHANT_ROUTE_SPAN;
            base + 1..=base + ENCHANT_ROUTE_LEVELS
        });
        (1..=self.levels).chain(enchants)
    }
}

struct ClientLevel {
    key: LevelKey,
    level: SkillLevel,
    notes: Vec<String>,
}

struct ClientName {
    key: LevelKey,
    name: Localized,
    description: Localized,
    enchant_name: Localized,
    enchant_description: Localized,
}

struct ClientSounds {
    key: LevelKey,
    sounds: SkillSounds,
    notes: Vec<String>,
}

pub(crate) fn migrate(sources: &Sources<'_>, report: &mut Report) -> BTreeMap<SkillId, Skill> {
    let server = server_index(sources.server_skills, "skill", report, server_skill);

    let levels = index("Skillgrp", sources.skills, client_level, |l| l.key, Severity::Error, report);
    let mut names = index("SkillName", sources.skill_names, client_name, |n| n.key, Severity::Error, report);
    let mut sounds = index("SkillSoundgrp", sources.skill_sounds, client_sounds, |s| s.key, Severity::Warning, report);

    let mut skills: BTreeMap<SkillId, Skill> = BTreeMap::new();
    let mut client_only = BTreeSet::new();
    for ((id, number), client) in levels {
        let Some(definition) = server.get(&id) else {
            client_only.insert(id);
            continue;
        };
        if !definition.defines(number) {
            report.push(Severity::Warning, subject(id), format!("level {number} is not defined by the server"));
            continue;
        }
        let mut level = client.level;
        if let Some(name) = names.remove(&(id, number)) {
            (level.name, level.description) = (name.name, name.description);
            (level.enchant_name, level.enchant_description) = (name.enchant_name, name.enchant_description);
        }
        if let Some(entry) = sounds.remove(&(id, number)) {
            level.sounds = entry.sounds;
            for note in entry.notes {
                report.push(Severity::Warning, subject(id), note);
            }
        }
        for note in client.notes {
            report.push(Severity::Warning, subject(id), note);
        }
        let skill =
            skills.entry(id).or_insert_with(|| Skill { id, operate: definition.operate, levels: BTreeMap::new() });
        skill.levels.insert(number, level);
    }

    for (&id, definition) in &server {
        let skill =
            skills.entry(id).or_insert_with(|| Skill { id, operate: definition.operate, levels: BTreeMap::new() });
        let missing: Vec<u32> = definition.all_levels().filter(|level| !skill.levels.contains_key(level)).collect();
        if !missing.is_empty() {
            report.push(
                Severity::Warning,
                subject(id),
                format!("{} levels without client presentation", missing.len()),
            );
        }
        for level in missing {
            skill.levels.insert(level, SkillLevel { name: Localized::en(&definition.name), ..SkillLevel::default() });
        }
    }
    for id in client_only {
        report.push(Severity::Warning, subject(id), "only in the client; not migrated");
    }
    for (id, _) in names.keys() {
        report.push(Severity::Warning, subject(*id), "name for a level with no presentation");
    }
    for (id, _) in sounds.keys() {
        report.push(Severity::Warning, subject(*id), "sounds for a level with no presentation");
    }
    skills
}

fn server_skill(node: Node<'_, '_>, report: &mut Report) -> Result<(SkillId, ServerSkill)> {
    let number = |name: &str| parse::<u32>(name, node.attribute(name).unwrap_or_default());
    let operate = node
        .children()
        .find(|child| child.has_tag_name("set") && child.attribute("name") == Some("operateType"))
        .and_then(|set| set.attribute("val"))
        .ok_or("missing operateType")?;
    let skill = ServerSkill {
        name: node.attribute("name").unwrap_or_default().to_owned(),
        levels: number("levels")?,
        // Routes are numbered contiguously from 1.
        routes: (1..=8u32)
            .filter(|route| node.attribute(format!("enchantGroup{route}").as_str()).is_some())
            .max()
            .unwrap_or(0),
        operate: skill_operate(operate).ok_or_else(|| format!("unknown operateType `{operate}`"))?,
    };
    for child in node.children().filter(Node::is_element) {
        let tag = child.tag_name().name();
        let tag = if tag.starts_with("enchant") { "enchantN" } else { tag };
        report.unmodeled(format!("skill <{tag}>"));
    }
    Ok((SkillId(number("id")?), skill))
}

fn skill_operate(name: &str) -> Option<SkillOperate> {
    Some(match name {
        "A1" => SkillOperate::A1,
        "A2" => SkillOperate::A2,
        "A3" => SkillOperate::A3,
        "A4" => SkillOperate::A4,
        "CA1" => SkillOperate::Ca1,
        "CA5" => SkillOperate::Ca5,
        "DA1" => SkillOperate::Da1,
        "DA2" => SkillOperate::Da2,
        "P" => SkillOperate::Passive,
        "T" => SkillOperate::Toggle,
        _ => return None,
    })
}

fn client_level(value: &Value) -> Result<ClientLevel> {
    let mut f = Fields::of(value)?;
    let mut assets = Assets::default();
    let key = (SkillId(f.uint("skill_id")?), f.uint("skill_level")?);
    let level = SkillLevel {
        icon: assets.one(f.text("icon")?),
        icon_panel: assets.one(f.text("icon_panel")?),
        enchant_icon: code(f.text("enchant_icon")?),
        animation: f.text("animation")?.to_owned(),
        effect: f.text("skill_visual_effect")?.to_owned(),
        ..SkillLevel::default()
    };
    f.ignore(GAMEPLAY_COPIES);
    f.ignore(UNKNOWN);
    f.finish()?;
    Ok(ClientLevel { key, level, notes: assets.notes })
}

fn client_name(value: &Value) -> Result<ClientName> {
    let mut f = Fields::of(value)?;
    let name = ClientName {
        key: (SkillId(f.uint("skill_id")?), f.uint("skill_level")?),
        name: localized(f.text("name")?),
        description: localized(f.text("desc")?),
        enchant_name: localized(f.text("enchant_name")?),
        enchant_description: localized(f.text("enchant_desc")?),
    };
    f.finish()?;
    Ok(name)
}

/// Voice columns per body, in table order.
const VOICE_BODIES: [(&str, Body); 16] = [
    ("mfighter", Body::HumanFighterMale),
    ("ffighter", Body::HumanFighterFemale),
    ("mmagic", Body::HumanMysticMale),
    ("fmagic", Body::HumanMysticFemale),
    ("melf", Body::ElfMale),
    ("felf", Body::ElfFemale),
    ("mdarkelf", Body::DarkElfMale),
    ("fdarkelf", Body::DarkElfFemale),
    ("mdwarf", Body::DwarfMale),
    ("fdwarf", Body::DwarfFemale),
    ("morc", Body::OrcFighterMale),
    ("forc", Body::OrcFighterFemale),
    ("mshaman", Body::OrcMysticMale),
    ("fshaman", Body::OrcMysticFemale),
    ("mkamael", Body::KamaelMale),
    ("fkamael", Body::KamaelFemale),
];

fn client_sounds(value: &Value) -> Result<ClientSounds> {
    let mut f = Fields::of(value)?;
    let mut assets = Assets::default();
    let key = (SkillId(f.uint("skill_id")?), f.uint("skill_level")?);
    let mut sounds = SkillSounds {
        spell: cues(&mut f, "spelleffect", &mut assets)?,
        shot: cues(&mut f, "shoteffect", &mut assets)?,
        explosion: cues(&mut f, "expeffect", &mut assets)?,
        ..SkillSounds::default()
    };
    for (suffix, voices) in [("cast", &mut sounds.cast_voices), ("magic", &mut sounds.magic_voices)] {
        for (column, body) in VOICE_BODIES {
            if let Some(sound) = assets.one(f.text(&format!("{column}_{suffix}"))?) {
                voices.insert(body, sound);
            }
        }
    }
    sounds.male_throw = assets.one(f.text("mextra_throw")?);
    sounds.female_throw = assets.one(f.text("fextra_throw")?);
    sounds.cast_volume = f.float("cast_volume")?;
    sounds.cast_radius = f.float("cast_rad")?;
    f.finish()?;
    Ok(ClientSounds { key, sounds, notes: assets.notes })
}

/// Up to three sounds with their volume and radius, e.g. `spelleffect_sound_1`.
fn cues(f: &mut Fields<'_>, prefix: &str, assets: &mut Assets) -> Result<Vec<SoundCue>> {
    let mut cues = Vec::new();
    for slot in 1..=3 {
        let sound = assets.one(f.text(&format!("{prefix}_sound_{slot}"))?);
        let volume = f.float(&format!("{prefix}_sound_vol_{slot}"))?;
        let radius = f.float(&format!("{prefix}_sound_rad_{slot}"))?;
        if let Some(sound) = sound {
            cues.push(SoundCue { sound, volume, radius });
        }
    }
    Ok(cues)
}

fn subject(id: SkillId) -> String {
    format!("skill {}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_and_enchant_levels() {
        let skill = ServerSkill { name: String::new(), levels: 3, routes: 2, operate: SkillOperate::A1 };
        for level in [1, 3, 101, 130, 201, 230] {
            assert!(skill.defines(level), "{level}");
        }
        for level in [0, 4, 100, 131, 200, 231, 301] {
            assert!(!skill.defines(level), "{level}");
        }
        assert_eq!(skill.all_levels().count(), 3 + 60);
        assert!(skill.all_levels().all(|level| skill.defines(level)));
    }
}

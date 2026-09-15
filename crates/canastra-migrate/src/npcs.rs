//! NPCs: identity, body and skills from the server; names and presentation from the client.

use std::collections::BTreeMap;
use std::str::FromStr;

use canastra_data::id::{NpcId, SkillId, SkillRef};
use canastra_data::npc::{Collision, Decoration, Npc, NpcFlags, NpcType, NpcVisual, Race, Sex};
use canastra_data::text::{Locale, Localized};
use l2_dat::Value;
use roxmltree::Node;

use crate::fields::{Assets, Fields, Result, localized, parse};
use crate::{Report, Severity, Sources, index, server_index};

/// Client copies of gameplay values; the server definition is authoritative.
const GAMEPLAY_COPIES: &[&str] = &["property_list"];
/// Values whose meaning is not known; nothing in the engine reads them yet.
const UNKNOWN: &[&str] = &["unk0_cnt", "unknown_2", "summon_sort"];
/// Data for systems that are not modeled yet.
const NOT_MODELED_YET: &[&str] = &["quest", "quest_step"];

/// Types the reference server lets die by default.
const MORTAL_TYPES: &[&str] = &["Monster", "RaidBoss", "GrandBoss"];

struct ClientNpc {
    id: NpcId,
    visual: NpcVisual,
    notes: Vec<String>,
}

struct ClientName {
    id: NpcId,
    name: Localized,
    title: Localized,
    title_color: u32,
}

pub(crate) fn migrate(sources: &Sources<'_>, report: &mut Report) -> BTreeMap<NpcId, Npc> {
    let mut npcs = server_index(sources.server_npcs, "npc", report, server_npc);

    let mut visuals = index("Npcgrp", sources.npcs, client_npc, |npc| npc.id, Severity::Error, report);
    let mut names = index("NpcName", sources.npc_names, client_name, |name| name.id, Severity::Error, report);
    for (&id, npc) in &mut npcs {
        match visuals.remove(&id) {
            Some(client) => {
                npc.visual = client.visual;
                for note in client.notes {
                    report.push(Severity::Warning, subject(id), note);
                }
            }
            None => report.push(Severity::Warning, subject(id), "no client presentation"),
        }
        if let Some(name) = names.remove(&id) {
            if name.name.get(Locale::En).is_some() {
                npc.name = name.name;
            }
            if name.title.get(Locale::En).is_some() {
                npc.title = name.title;
            }
            npc.title_color = name.title_color;
        }
    }
    for id in visuals.keys().chain(names.keys()) {
        report.push(Severity::Warning, subject(*id), "only in the client; not migrated");
    }
    npcs
}

/// What the reference server assumes for an NPC that leaves these out, as custom NPCs often do.
const DEFAULT_LEVEL: u32 = 85;
const DEFAULT_WALK_SPEED: f64 = 50.0;
const DEFAULT_RUN_SPEED: f64 = 120.0;

fn server_npc(node: Node<'_, '_>, report: &mut Report) -> Result<(NpcId, Npc)> {
    let npc_type = node.attribute("type").unwrap_or_default();
    let child = |tag: &str| node.children().find(|child| child.has_tag_name(tag));
    let path = |tags: &[&str]| {
        tags.iter().try_fold(node, |current, tag| current.children().find(|child| child.has_tag_name(*tag)))
    };
    let status = child("status");
    let flag = |name: &str, default: bool| -> Result<bool> {
        status.and_then(|status| status.attribute(name)).map_or(Ok(default), |value| parse(name, value))
    };

    let npc = Npc {
        id: NpcId(parse("id", node.attribute("id").unwrap_or_default())?),
        name: localized(node.attribute("name").unwrap_or_default()),
        title: localized(node.attribute("title").unwrap_or_default()),
        title_color: 0,
        npc_type: NpcType(npc_type.to_owned()),
        level: optional(node, "level")?.unwrap_or(DEFAULT_LEVEL),
        race: child("race")
            .and_then(|race| race.text())
            .map_or(Ok(Race::None), |text| race_of(text).ok_or_else(|| format!("unknown race `{text}`")))?,
        sex: child("sex")
            .and_then(|sex| sex.text())
            .map_or(Ok(Sex::Etc), |text| sex_of(text).ok_or_else(|| format!("unknown sex `{text}`")))?,
        hp: defaulted(path(&["stats", "vitals"]), "hp", 0.0)?,
        mp: defaulted(path(&["stats", "vitals"]), "mp", 0.0)?,
        walk_speed: defaulted(path(&["stats", "speed", "walk"]), "ground", DEFAULT_WALK_SPEED)?,
        run_speed: defaulted(path(&["stats", "speed", "run"]), "ground", DEFAULT_RUN_SPEED)?,
        collision: Collision {
            radius: required(path(&["collision", "radius"]), "normal")?,
            height: required(path(&["collision", "height"]), "normal")?,
            grown: match (path(&["collision", "radius"]), path(&["collision", "height"])) {
                (Some(radius), Some(height)) => optional(radius, "grown")?.zip(optional(height, "grown")?),
                _ => None,
            },
        },
        skills: child("skillList").map(skill_list).transpose()?.unwrap_or_default(),
        flags: NpcFlags {
            attackable: flag("attackable", true)?,
            targetable: flag("targetable", true)?,
            talkable: flag("talkable", true)?,
            show_name: flag("showName", true)?,
            undying: flag("undying", !MORTAL_TYPES.contains(&npc_type))?,
            flying: flag("flying", false)?,
            can_move: flag("canMove", true)?,
        },
        visual: NpcVisual::default(),
    };

    let modeled_status = ["attackable", "targetable", "talkable", "showName", "undying", "flying", "canMove"];
    for element in node.children().filter(Node::is_element) {
        match element.tag_name().name() {
            "race" | "sex" | "collision" | "skillList" => {}
            "status" => element
                .attributes()
                .filter(|attribute| !modeled_status.contains(&attribute.name()))
                .for_each(|attribute| report.unmodeled(format!("npc status@{}", attribute.name()))),
            "stats" => stats_unmodeled(element, report),
            other => report.unmodeled(format!("npc <{other}>")),
        }
    }
    Ok((npc.id, npc))
}

/// Everything under `<stats>` except vitals and ground speeds.
fn stats_unmodeled(stats: Node<'_, '_>, report: &mut Report) {
    stats.attributes().for_each(|attribute| report.unmodeled(format!("npc stats@{}", attribute.name())));
    for element in stats.children().filter(Node::is_element) {
        match element.tag_name().name() {
            "vitals" => element
                .attributes()
                .filter(|attribute| !["hp", "mp"].contains(&attribute.name()))
                .for_each(|attribute| report.unmodeled(format!("npc vitals@{}", attribute.name()))),
            "speed" => {
                for movement in element.children().filter(Node::is_element) {
                    movement.attributes().filter(|attribute| attribute.name() != "ground").for_each(|attribute| {
                        report.unmodeled(format!("npc speed/{}@{}", movement.tag_name().name(), attribute.name()));
                    });
                }
            }
            other => report.unmodeled(format!("npc <stats/{other}>")),
        }
    }
}

fn skill_list(list: Node<'_, '_>) -> Result<Vec<SkillRef>> {
    list.children()
        .filter(|child| child.has_tag_name("skill"))
        .map(|skill| {
            Ok(SkillRef {
                id: SkillId(parse("skill id", skill.attribute("id").unwrap_or_default())?),
                level: parse("skill level", skill.attribute("level").unwrap_or_default())?,
            })
        })
        .collect()
}

fn required<T: FromStr>(node: Option<Node<'_, '_>>, attribute: &str) -> Result<T> {
    let node = node.ok_or_else(|| format!("missing element for `{attribute}`"))?;
    optional(node, attribute)?.ok_or_else(|| format!("missing `{attribute}` on <{}>", node.tag_name().name()))
}

/// The attribute, or what the reference server assumes when the element or attribute is left out.
fn defaulted<T: FromStr>(node: Option<Node<'_, '_>>, attribute: &str, default: T) -> Result<T> {
    Ok(node.map(|node| optional(node, attribute)).transpose()?.flatten().unwrap_or(default))
}

fn optional<T: FromStr>(node: Node<'_, '_>, attribute: &str) -> Result<Option<T>> {
    node.attribute(attribute).map(|value| parse(attribute, value)).transpose()
}

fn race_of(name: &str) -> Option<Race> {
    Some(match name {
        "NONE" => Race::None,
        "HUMAN" => Race::Human,
        "ELF" => Race::Elf,
        "DARK_ELF" => Race::DarkElf,
        "ORC" => Race::Orc,
        "DWARF" => Race::Dwarf,
        "KAMAEL" => Race::Kamael,
        "HUMANOID" => Race::Humanoid,
        "ANIMAL" => Race::Animal,
        "BEAST" => Race::Beast,
        "BUG" => Race::Bug,
        "PLANT" => Race::Plant,
        "CONSTRUCT" => Race::Construct,
        "UNDEAD" => Race::Undead,
        "DEMONIC" => Race::Demonic,
        "DIVINE" => Race::Divine,
        "DRAGON" => Race::Dragon,
        "ELEMENTAL" => Race::Elemental,
        "FAIRY" => Race::Fairy,
        "GIANT" => Race::Giant,
        "CASTLE_GUARD" => Race::CastleGuard,
        "MERCENARY" => Race::Mercenary,
        "SIEGE_WEAPON" => Race::SiegeWeapon,
        "ETC" => Race::Etc,
        _ => return None,
    })
}

fn sex_of(name: &str) -> Option<Sex> {
    Some(match name {
        "MALE" => Sex::Male,
        "FEMALE" => Sex::Female,
        "ETC" => Sex::Etc,
        _ => return None,
    })
}

fn client_npc(value: &Value) -> Result<ClientNpc> {
    let mut f = Fields::of(value)?;
    let mut assets = Assets::default();
    let id = NpcId(f.uint("npc_id")?);
    let mut visual = NpcVisual {
        class: assets.one(f.text("class_name")?),
        mesh: assets.one(f.text("mesh_name")?),
        textures: assets.many(&f.texts("texture_name")?),
        extra_textures: assets.many(&f.texts("texture_name_second")?),
        speed_scale: f.float("npc_speed")?,
        attack_sounds: assets.many(&f.texts("attack_sound1")?),
        defense_sounds: assets.many(&f.texts("defense_sound1")?),
        damage_sounds: assets.many(&f.texts("damage_sound")?),
        dialog_sounds: f.texts("dialog_sound")?.into_iter().map(str::to_owned).collect(),
        attack_effect: assets.one(f.text("attack_effect")?),
        sound_volume: f.float("sound_vol")?,
        sound_radius: f.float("sound_radius")?,
        sound_random: f.float("sound_random")?,
        hp_bar: f.flag("hpshowable")?,
        social: f.flag("social")?,
        ..NpcVisual::default()
    };
    for entry in f.list("deco_effect")? {
        let mut deco = Fields::of(entry)?;
        let effect = assets.one(deco.text("param_deco_effect")?);
        let scale = deco.float("param_deco_effect_scale")?;
        deco.finish()?;
        if let Some(effect) = effect {
            visual.decorations.push(Decoration { effect, scale });
        }
    }
    f.ignore(GAMEPLAY_COPIES);
    f.ignore(UNKNOWN);
    f.ignore(NOT_MODELED_YET);
    f.finish()?;
    Ok(ClientNpc { id, visual, notes: assets.notes })
}

fn client_name(value: &Value) -> Result<ClientName> {
    let mut f = Fields::of(value)?;
    let name = ClientName {
        id: NpcId(f.uint("id")?),
        name: localized(f.text("name")?),
        title: localized(f.text("nick")?),
        title_color: f.uint("nickcolor")?,
    };
    f.finish()?;
    Ok(name)
}

fn subject(id: NpcId) -> String {
    format!("npc {}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NPC: &str = r#"<list>
        <npc id="100" level="80" type="Folk" name="Thomas D. Turkey">
            <race>ANIMAL</race>
            <sex>MALE</sex>
            <stats str="40">
                <vitals hp="680" mp="2000.0" mpRegen="0.9" />
                <attack physical="8.4" />
                <speed><walk ground="19.5" /><run ground="160" /></speed>
            </stats>
            <status attackable="false" canBeSown="true" />
            <skillList><skill id="4390" level="1" /></skillList>
            <ai aggroRange="300" />
            <collision><radius normal="25" grown="30" /><height normal="35" grown="40" /></collision>
        </npc>
    </list>"#;

    #[test]
    fn maps_body_and_reports_the_rest() {
        let document = roxmltree::Document::parse(NPC).unwrap();
        let node = document.root_element().children().find(|node| node.has_tag_name("npc")).unwrap();
        let mut report = Report::default();
        let (_, npc) = server_npc(node, &mut report).unwrap();

        assert_eq!((npc.level, npc.race, npc.sex), (80, Race::Animal, Sex::Male));
        assert_eq!((npc.hp, npc.walk_speed, npc.run_speed), (680.0, 19.5, 160.0));
        assert_eq!(npc.collision, Collision { radius: 25.0, height: 35.0, grown: Some((30.0, 40.0)) });
        assert!(!npc.flags.attackable && npc.flags.undying && npc.flags.targetable);
        assert_eq!(npc.skills, [SkillRef { id: SkillId(4390), level: 1 }]);
        assert_eq!(
            report.unmodeled.keys().collect::<Vec<_>>(),
            ["npc <ai>", "npc <stats/attack>", "npc stats@str", "npc status@canBeSown", "npc vitals@mpRegen"]
        );
    }
}

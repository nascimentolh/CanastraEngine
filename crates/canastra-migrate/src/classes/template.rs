//! One `baseStats/<Class>.xml` document: a class's static template, creation points and level gains.

use std::str::FromStr;

use canastra_data::class::{
    Attributes, BodySize, ClassTemplate, CombatBase, DamageRange, LevelGain, MDefSlots, MoveSpeed, PDefSlots,
};
use canastra_data::id::ClassId;
use canastra_data::item::WeaponType;
use roxmltree::Node;

use crate::Report;
use crate::fields::{Result, parse};

/// Everything a template document says about one class.
pub(super) struct Parsed {
    pub(super) class: ClassId,
    pub(super) template: ClassTemplate,
    pub(super) creation_points: Vec<[i32; 3]>,
    pub(super) levels: Vec<LevelGain>,
}

/// Elements of `<staticData>` this parser reads; any other is counted as unmodeled.
const STATIC: &[&str] = &[
    "baseINT",
    "baseSTR",
    "baseCON",
    "baseMEN",
    "baseDEX",
    "baseWIT",
    "creationPoints",
    "basePAtk",
    "baseCritRate",
    "baseAtkType",
    "basePAtkSpd",
    "basePDef",
    "baseMAtk",
    "baseMDef",
    "baseCanPenetrate",
    "baseAtkRange",
    "baseDamRange",
    "baseRndDam",
    "baseMoveSpd",
    "baseBreath",
    "baseSafeFall",
    "collisionMale",
    "collisionFemale",
];

pub(super) fn parse_document(xml: &str, report: &mut Report) -> Result<Parsed> {
    let document = roxmltree::Document::parse(xml).map_err(|error| error.to_string())?;
    let root = document.root_element();
    let class = ClassId(text(child(root, "classId")?)?);
    let data = child(root, "staticData")?;
    for element in data.children().filter(Node::is_element) {
        if !STATIC.contains(&element.tag_name().name()) {
            report.unmodeled(format!("class template <{}>", element.tag_name().name()));
        }
    }
    let value = |path: &[&str]| -> Result<Node<'_, '_>> { path.iter().try_fold(data, |node, tag| child(node, tag)) };
    let number = |path: &[&str]| -> Result<f64> { text(value(path)?) };
    let body = |tag: &str| -> Result<BodySize> {
        Ok(BodySize { radius: number(&[tag, "radius"])?, height: number(&[tag, "height"])? })
    };
    let attack_type = text::<String>(value(&["baseAtkType"])?)?;
    let template = ClassTemplate {
        attributes: Attributes {
            str: text(value(&["baseSTR"])?)?,
            dex: text(value(&["baseDEX"])?)?,
            con: text(value(&["baseCON"])?)?,
            int: text(value(&["baseINT"])?)?,
            wit: text(value(&["baseWIT"])?)?,
            men: text(value(&["baseMEN"])?)?,
        },
        combat: CombatBase {
            p_atk: number(&["basePAtk"])?,
            m_atk: number(&["baseMAtk"])?,
            critical_rate: number(&["baseCritRate"])?,
            attack_type: WeaponType::ALL
                .iter()
                .copied()
                .find(|kind| format!("{kind:?}").eq_ignore_ascii_case(&attack_type.replace('_', "")))
                .ok_or_else(|| format!("unknown attack type `{attack_type}`"))?,
            p_atk_speed: number(&["basePAtkSpd"])?,
            p_def: PDefSlots {
                chest: number(&["basePDef", "chest"])?,
                legs: number(&["basePDef", "legs"])?,
                head: number(&["basePDef", "head"])?,
                feet: number(&["basePDef", "feet"])?,
                gloves: number(&["basePDef", "gloves"])?,
                underwear: number(&["basePDef", "underwear"])?,
                cloak: number(&["basePDef", "cloak"])?,
            },
            m_def: MDefSlots {
                right_ear: number(&["baseMDef", "rear"])?,
                left_ear: number(&["baseMDef", "lear"])?,
                right_finger: number(&["baseMDef", "rfinger"])?,
                left_finger: number(&["baseMDef", "lfinger"])?,
                neck: number(&["baseMDef", "neck"])?,
            },
            penetrates: text::<u8>(value(&["baseCanPenetrate"])?)? != 0,
            attack_range: text(value(&["baseAtkRange"])?)?,
            damage_range: DamageRange {
                vertical: text(value(&["baseDamRange", "verticalDirection"])?)?,
                horizontal: text(value(&["baseDamRange", "horizontalDirection"])?)?,
                distance: text(value(&["baseDamRange", "distance"])?)?,
                width: text(value(&["baseDamRange", "width"])?)?,
            },
            random_damage: text(value(&["baseRndDam"])?)?,
        },
        move_speed: MoveSpeed {
            walk: number(&["baseMoveSpd", "walk"])?,
            run: number(&["baseMoveSpd", "run"])?,
            slow_swim: number(&["baseMoveSpd", "slowSwim"])?,
            fast_swim: number(&["baseMoveSpd", "fastSwim"])?,
        },
        breath: text(value(&["baseBreath"])?)?,
        safe_fall: text(value(&["baseSafeFall"])?)?,
        body_male: body("collisionMale")?,
        body_female: body("collisionFemale")?,
    };
    let creation_points = value(&["creationPoints"])?
        .children()
        .filter(|node| node.has_tag_name("node"))
        .map(|node| {
            let coordinate = |name: &str| parse::<i32>(name, node.attribute(name).unwrap_or_default());
            Ok([coordinate("x")?, coordinate("y")?, coordinate("z")?])
        })
        .collect::<Result<_>>()?;
    Ok(Parsed { class, template, creation_points, levels: levels(child(root, "lvlUpgainData")?)? })
}

/// Level gains in order; levels must run from 1 without gaps.
fn levels(gains: Node<'_, '_>) -> Result<Vec<LevelGain>> {
    let mut levels = Vec::new();
    for level in gains.children().filter(|node| node.has_tag_name("level")) {
        let number: usize = parse("level val", level.attribute("val").unwrap_or_default())?;
        if number != levels.len() + 1 {
            return Err(format!("level {number} is out of order; expected {}", levels.len() + 1));
        }
        let gain = |tag: &str| -> Result<f64> { text(child(level, tag)?) };
        levels.push(LevelGain {
            hp: gain("hp")?,
            mp: gain("mp")?,
            cp: gain("cp")?,
            hp_regen: gain("hpRegen")?,
            mp_regen: gain("mpRegen")?,
            cp_regen: gain("cpRegen")?,
        });
    }
    Ok(levels)
}

fn child<'a, 'input>(node: Node<'a, 'input>, tag: &str) -> Result<Node<'a, 'input>> {
    node.children().find(|child| child.has_tag_name(tag)).ok_or_else(|| format!("missing <{tag}>"))
}

fn text<T: FromStr>(node: Node<'_, '_>) -> Result<T> {
    parse(node.tag_name().name(), node.text().unwrap_or_default())
}

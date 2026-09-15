//! Canastra game data: the single domain model shared by client, server and Studio.
//!
//! Types here describe what the game means, not how any legacy file stored it.
//! Gameplay and presentation live together so both sides always agree.

/// Declares a fieldless enum plus `ALL`, its variants in declaration order, so editors offer
/// every choice from the same list the compiler sees.
macro_rules! choices {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $($(#[$variant_meta:meta])* $variant:ident),+ $(,)? }) => {
        $(#[$meta])* $vis enum $name { $($(#[$variant_meta])* $variant),+ }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}

pub mod asset;
pub mod class;
pub mod format;
pub mod id;
pub mod item;
pub mod npc;
pub mod skill;
pub mod text;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use class::{Origin, PlayerClass};
use id::{ClassId, ItemId, NpcId, SkillId, SkillRef};
use item::{Item, ItemKind};
use npc::Npc;
use skill::Skill;
use text::Locale;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GameData {
    pub items: BTreeMap<ItemId, Item>,
    pub skills: BTreeMap<SkillId, Skill>,
    pub npcs: BTreeMap<NpcId, Npc>,
    pub classes: BTreeMap<ClassId, PlayerClass>,
}

/// A rule the data breaks. Studio refuses to save while any exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub subject: Subject,
    pub problem: Problem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    Item(ItemId),
    Npc(NpcId),
    Class(ClassId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    MissingName,
    WeaponOutsideHands,
    StackableEquipment,
    UnknownSkill(SkillRef),
    UnknownItem(ItemId),
    UnknownClass(ClassId),
}

impl GameData {
    pub fn validate(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let known = |skill: &SkillRef| self.skills.get(&skill.id).is_some_and(|s| s.levels.contains_key(&skill.level));
        for (&id, item) in &self.items {
            let mut report = |problem| issues.push(Issue { subject: Subject::Item(id), problem });
            if item.name.get(Locale::En).is_none_or(str::is_empty) {
                report(Problem::MissingName);
            }
            if let ItemKind::Weapon(weapon) = &item.kind
                && !weapon.slot.is_hand()
            {
                report(Problem::WeaponOutsideHands);
            }
            if item.flags.stackable && !matches!(item.kind, ItemKind::Etc(_)) {
                report(Problem::StackableEquipment);
            }
            item.skills.iter().filter(|skill| !known(skill)).for_each(|&skill| report(Problem::UnknownSkill(skill)));
        }
        for (&id, npc) in &self.npcs {
            for &skill in npc.skills.iter().filter(|skill| !known(skill)) {
                issues.push(Issue { subject: Subject::Npc(id), problem: Problem::UnknownSkill(skill) });
            }
        }
        for (&id, class) in &self.classes {
            let mut report = |problem| issues.push(Issue { subject: Subject::Class(id), problem });
            match &class.origin {
                Origin::Advanced { parent } if !self.classes.contains_key(parent) => {
                    report(Problem::UnknownClass(*parent));
                }
                Origin::Advanced { .. } => {}
                Origin::Starting(start) => {
                    for initial in start.initial_items.iter().filter(|initial| !self.items.contains_key(&initial.item))
                    {
                        report(Problem::UnknownItem(initial.item));
                    }
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use item::{EquipSlot, ItemFlags, Weapon, WeaponType};
    use skill::{SkillLevel, SkillOperate};
    use text::Localized;

    fn data() -> GameData {
        let sword = Item {
            name: Localized::en("Short Sword"),
            kind: ItemKind::Weapon(Weapon {
                weapon_type: WeaponType::Sword,
                slot: EquipSlot::RightHand,
                ..Weapon::default()
            }),
            skills: vec![SkillRef { id: SkillId(3), level: 1 }],
            ..Item::new(ItemId(1))
        };
        let skill =
            Skill { id: SkillId(3), operate: SkillOperate::A1, levels: BTreeMap::from([(1, SkillLevel::default())]) };
        GameData {
            items: BTreeMap::from([(ItemId(1), sword)]),
            skills: BTreeMap::from([(SkillId(3), skill)]),
            npcs: BTreeMap::new(),
            classes: BTreeMap::new(),
        }
    }

    #[test]
    fn valid_data_has_no_issues() {
        assert!(data().validate().is_empty());
    }

    #[test]
    fn reports_broken_rules() {
        let mut data = data();
        let item = data.items.get_mut(&ItemId(1)).unwrap();
        item.name = Localized::default();
        item.flags = ItemFlags { stackable: true, ..ItemFlags::default() };
        item.skills[0].level = 2;
        if let ItemKind::Weapon(weapon) = &mut item.kind {
            weapon.slot = EquipSlot::Head;
        }
        let problems: Vec<_> = data.validate().into_iter().map(|issue| issue.problem).collect();
        assert_eq!(
            problems,
            [
                Problem::MissingName,
                Problem::WeaponOutsideHands,
                Problem::StackableEquipment,
                Problem::UnknownSkill(SkillRef { id: SkillId(3), level: 2 }),
            ]
        );
    }
}

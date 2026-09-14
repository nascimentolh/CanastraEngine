//! Canastra game data: the single domain model shared by client, server and Studio.
//!
//! Types here describe what the game means, not how any legacy file stored it.
//! Gameplay and presentation live together so both sides always agree.

pub mod asset;
pub mod id;
pub mod item;
pub mod text;

use std::collections::BTreeMap;

use id::ItemId;
use item::Item;
use text::Locale;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameData {
    pub items: BTreeMap<ItemId, Item>,
}

/// A rule the data breaks. Studio refuses to save while any exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub item: ItemId,
    pub problem: Problem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    MissingName,
    WeaponOutsideHands,
    StackableEquipment,
}

impl GameData {
    pub fn validate(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (&id, item) in &self.items {
            let mut report = |problem| issues.push(Issue { item: id, problem });
            if item.name.get(Locale::En).is_none_or(str::is_empty) {
                report(Problem::MissingName);
            }
            if let item::ItemKind::Weapon(weapon) = &item.kind
                && !weapon.slot.is_hand()
            {
                report(Problem::WeaponOutsideHands);
            }
            if item.flags.stackable && !matches!(item.kind, item::ItemKind::Etc(_)) {
                report(Problem::StackableEquipment);
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use item::{EquipSlot, ItemFlags, ItemKind, Weapon, WeaponType};
    use text::Localized;

    fn sword() -> Item {
        Item {
            name: Localized::en("Short Sword"),
            kind: ItemKind::Weapon(Weapon {
                weapon_type: WeaponType::Sword,
                slot: EquipSlot::RightHand,
                ..Weapon::default()
            }),
            ..Item::new(ItemId(1))
        }
    }

    #[test]
    fn valid_item_has_no_issues() {
        let data = GameData { items: BTreeMap::from([(ItemId(1), sword())]) };
        assert!(data.validate().is_empty());
    }

    #[test]
    fn reports_broken_rules() {
        let mut item = sword();
        item.name = Localized::default();
        item.flags = ItemFlags { stackable: true, ..ItemFlags::default() };
        if let ItemKind::Weapon(weapon) = &mut item.kind {
            weapon.slot = EquipSlot::Head;
        }
        let data = GameData { items: BTreeMap::from([(ItemId(1), item)]) };
        let problems: Vec<_> = data.validate().into_iter().map(|issue| issue.problem).collect();
        assert_eq!(problems, [Problem::MissingName, Problem::WeaponOutsideHands, Problem::StackableEquipment]);
    }
}

//! Player classes. Characters are created as a starting class, which holds everything its line shares:
//! race, archetype, body, base stats, where characters appear and what they carry. Advanced classes are
//! chosen from a parent and only add what differs, their level gains.

use serde::{Deserialize, Serialize};

use crate::id::{ClassId, ItemId};
use crate::item::WeaponType;
use crate::npc::{Race, Sex};
use crate::text::Localized;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerClass {
    pub id: ClassId,
    pub name: Localized,
    /// What each level gives, from level 1.
    pub levels: Vec<LevelGain>,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Origin {
    /// Characters are created as this class.
    Starting(Box<StartingClass>),
    /// Chosen from `parent`; race, template and starting data come from the line's starting class.
    Advanced { parent: ClassId },
}

/// What a line of classes shares, kept once on the class it starts from.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StartingClass {
    pub race: Race,
    pub archetype: Archetype,
    /// The only sex characters of this line may be, for lines tied to one.
    pub sex: Option<Sex>,
    /// What the creation screen tells of the class: a line in its voice, then the path it follows.
    pub description: Localized,
    pub template: ClassTemplate,
    /// Where new characters appear.
    pub creation_points: Vec<[i32; 3]>,
    /// What new characters carry, in the order they receive it. The server owner sets this in Studio; the
    /// migrated kit is only a suggested default.
    pub initial_items: Vec<InitialItem>,
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
    pub enum Archetype {
        #[default]
        Fighter,
        Mystic,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ClassTemplate {
    pub attributes: Attributes,
    pub combat: CombatBase,
    pub move_speed: MoveSpeed,
    /// Seconds of breath under water.
    pub breath: u32,
    /// Height a character falls without damage, in world units.
    pub safe_fall: u32,
    pub body_male: BodySize,
    pub body_female: BodySize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Attributes {
    pub str: u32,
    pub dex: u32,
    pub con: u32,
    pub int: u32,
    pub wit: u32,
    pub men: u32,
}

/// Combat values before equipment, skills and attributes apply.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct CombatBase {
    pub p_atk: f64,
    pub m_atk: f64,
    pub critical_rate: f64,
    pub attack_type: WeaponType,
    pub p_atk_speed: f64,
    pub p_def: PDefSlots,
    pub m_def: MDefSlots,
    /// Whether attacks pass through a target to hit what stands behind it.
    pub penetrates: bool,
    pub attack_range: u32,
    pub damage_range: DamageRange,
    /// Percent the damage of an attack varies.
    pub random_damage: u32,
}

/// Physical defense each unequipped armor slot gives.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PDefSlots {
    pub chest: f64,
    pub legs: f64,
    pub head: f64,
    pub feet: f64,
    pub gloves: f64,
    pub underwear: f64,
    pub cloak: f64,
}

/// Magic defense each unequipped jewelry slot gives.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct MDefSlots {
    pub right_ear: f64,
    pub left_ear: f64,
    pub right_finger: f64,
    pub left_finger: f64,
    pub neck: f64,
}

/// The area an unarmed attack reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DamageRange {
    pub vertical: i32,
    pub horizontal: i32,
    pub distance: i32,
    pub width: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct MoveSpeed {
    pub walk: f64,
    pub run: f64,
    pub slow_swim: f64,
    pub fast_swim: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct BodySize {
    pub radius: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct LevelGain {
    pub hp: f64,
    pub mp: f64,
    pub cp: f64,
    pub hp_regen: f64,
    pub mp_regen: f64,
    pub cp_regen: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitialItem {
    pub item: ItemId,
    pub count: u64,
    pub equipped: bool,
    /// Minutes the item lasts from character creation, then disappears; `None` keeps it forever. The
    /// suggested recruit kit gives top no-grade gear for 48 hours over permanent starter gear.
    pub lasts_minutes: Option<u32>,
}

//! NPCs: identity, body and presentation. Combat, AI and drops arrive with the server design.

use serde::{Deserialize, Serialize};

use crate::asset::{ClassRef, EffectRef, MeshRef, SoundRef, TextureRef};
use crate::id::{NpcId, SkillRef};
use crate::text::Localized;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Npc {
    pub id: NpcId,
    pub name: Localized,
    pub title: Localized,
    /// `0xAARRGGBB`.
    pub title_color: u32,
    pub npc_type: NpcType,
    pub level: u32,
    pub race: Race,
    pub sex: Sex,
    pub hp: f64,
    pub mp: f64,
    pub walk_speed: f64,
    pub run_speed: f64,
    pub collision: Collision,
    pub skills: Vec<SkillRef>,
    pub flags: NpcFlags,
    pub visual: NpcVisual,
}

/// Server behavior registered under this key, e.g. `Merchant`; plugins may add their own.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NpcType(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Collision {
    pub radius: f64,
    pub height: f64,
    /// Size after growing, for NPCs that grow.
    pub grown: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[expect(clippy::struct_excessive_bools, reason = "independent rule switches edited as checkboxes")]
pub struct NpcFlags {
    pub attackable: bool,
    pub targetable: bool,
    pub talkable: bool,
    pub show_name: bool,
    pub undying: bool,
    pub flying: bool,
    pub can_move: bool,
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
    pub enum Race {
        #[default]
        None,
        Human,
        Elf,
        DarkElf,
        Orc,
        Dwarf,
        Kamael,
        Humanoid,
        Animal,
        Beast,
        Bug,
        Plant,
        Construct,
        Undead,
        Demonic,
        Divine,
        Dragon,
        Elemental,
        Fairy,
        Giant,
        CastleGuard,
        Mercenary,
        SiegeWeapon,
        Etc,
    }
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
    pub enum Sex {
        Male,
        Female,
        #[default]
        Etc,
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NpcVisual {
    pub class: Option<ClassRef>,
    pub mesh: Option<MeshRef>,
    pub textures: Vec<TextureRef>,
    pub extra_textures: Vec<TextureRef>,
    /// Client movement animation rate.
    pub speed_scale: f32,
    pub attack_sounds: Vec<SoundRef>,
    pub defense_sounds: Vec<SoundRef>,
    pub damage_sounds: Vec<SoundRef>,
    /// Voice line names, e.g. `elcardia_greeting_1`.
    pub dialog_sounds: Vec<String>,
    pub decorations: Vec<Decoration>,
    pub attack_effect: Option<EffectRef>,
    pub sound_volume: f32,
    pub sound_radius: f32,
    pub sound_random: f32,
    pub hp_bar: bool,
    pub social: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decoration {
    pub effect: EffectRef,
    pub scale: f32,
}

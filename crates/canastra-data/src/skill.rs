//! Skills: what each level is called and how it looks and sounds.
//! Mechanics (costs, effects, conditions) arrive with the server design.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::asset::{SoundRef, TextureRef};
use crate::id::SkillId;
use crate::item::Body;
use crate::text::Localized;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    pub id: SkillId,
    pub operate: SkillOperate,
    /// Regular levels, then enchant routes at 101+.
    pub levels: BTreeMap<u32, SkillLevel>,
}

/// Operate types as named by the reference server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SkillOperate {
    A1,
    A2,
    A3,
    A4,
    Ca1,
    Ca5,
    Da1,
    Da2,
    Passive,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SkillLevel {
    pub name: Localized,
    pub description: Localized,
    pub enchant_name: Localized,
    pub enchant_description: Localized,
    pub icon: Option<TextureRef>,
    pub icon_panel: Option<TextureRef>,
    /// Client code of the enchant route icon, e.g. `power01`.
    pub enchant_icon: String,
    /// Client animation code, e.g. `S`.
    pub animation: String,
    /// Client visual effect id.
    pub effect: String,
    pub sounds: SkillSounds,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SkillSounds {
    pub spell: Vec<SoundCue>,
    pub shot: Vec<SoundCue>,
    pub explosion: Vec<SoundCue>,
    pub cast_voices: BTreeMap<Body, SoundRef>,
    pub magic_voices: BTreeMap<Body, SoundRef>,
    pub male_throw: Option<SoundRef>,
    pub female_throw: Option<SoundRef>,
    pub cast_volume: f32,
    pub cast_radius: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundCue {
    pub sound: SoundRef,
    pub volume: f32,
    pub radius: f32,
}

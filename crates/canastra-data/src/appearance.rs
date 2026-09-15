//! How characters look apart from their gear, and where the lobby stands them.

use serde::{Deserialize, Serialize};

use crate::asset::{MeshRef, TextureRef};
use crate::class::Archetype;
use crate::id::ItemId;
use crate::item::Body;
use crate::npc::Race;

/// A skinned mesh and the texture each of its sections wears.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Look {
    pub mesh: MeshRef,
    pub textures: Vec<TextureRef>,
}

/// One hair style: the hair around the face and the hair behind, either of which a style may lack.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HairStyle {
    pub front: Option<Look>,
    pub back: Option<Look>,
}

/// A body before any gear: the faces and hair styles a character picks from, and the parts it wears where
/// no armor covers it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BodyLook {
    pub faces: Vec<Look>,
    pub hair_styles: Vec<HairStyle>,
    pub gloves: Option<Look>,
    pub upper: Option<Look>,
    pub lower: Option<Look>,
    pub boots: Option<Look>,
}

/// A place in a lobby scene: where a character stands, in world units, and the way it faces.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stand {
    pub location: [f32; 3],
    /// Unreal rotation units, 65536 to a full turn.
    pub yaw: i32,
}

/// A character shown in the creation scene, wearing gear that shows off its race and build.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayCharacter {
    pub body: Body,
    pub archetype: Archetype,
    pub stand: Stand,
    pub gear: Vec<ItemId>,
}

/// Where the lobby's scenes stand characters.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Lobby {
    /// The character select slots; the last is the selected character's place.
    pub select: Vec<Stand>,
    pub creation: Vec<DisplayCharacter>,
}

/// The body of a character of `race` and `archetype`. Humans and Orcs have separate bodies for fighters and
/// mystics; the other races share one per sex.
pub fn body_of(race: Race, archetype: Archetype, female: bool) -> Option<Body> {
    let mystic = archetype == Archetype::Mystic;
    Some(match (race, mystic, female) {
        (Race::Human, false, false) => Body::HumanFighterMale,
        (Race::Human, false, true) => Body::HumanFighterFemale,
        (Race::Human, true, false) => Body::HumanMysticMale,
        (Race::Human, true, true) => Body::HumanMysticFemale,
        (Race::Elf, _, false) => Body::ElfMale,
        (Race::Elf, _, true) => Body::ElfFemale,
        (Race::DarkElf, _, false) => Body::DarkElfMale,
        (Race::DarkElf, _, true) => Body::DarkElfFemale,
        (Race::Orc, false, false) => Body::OrcFighterMale,
        (Race::Orc, false, true) => Body::OrcFighterFemale,
        (Race::Orc, true, false) => Body::OrcMysticMale,
        (Race::Orc, true, true) => Body::OrcMysticFemale,
        (Race::Dwarf, _, false) => Body::DwarfMale,
        (Race::Dwarf, _, true) => Body::DwarfFemale,
        (Race::Kamael, _, false) => Body::KamaelMale,
        (Race::Kamael, _, true) => Body::KamaelFemale,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elves_share_a_body_and_humans_split_by_archetype() {
        assert_eq!(body_of(Race::Elf, Archetype::Mystic, true), body_of(Race::Elf, Archetype::Fighter, true));
        assert_eq!(body_of(Race::Human, Archetype::Mystic, false), Some(Body::HumanMysticMale));
        assert_eq!(body_of(Race::Beast, Archetype::Fighter, false), None);
    }
}

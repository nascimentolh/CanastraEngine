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

impl HairStyle {
    /// The style in hair color `color`, from 0. The client keeps each color as the style's textures numbered
    /// after the first, `_t00_` becoming `_t01_` to `_t03_` (`Hair_color.MFighter_m000_t01_m00_ah`).
    #[must_use]
    pub fn colored(&self, color: u8) -> Self {
        let number = format!("_t{color:02}_");
        let look = |look: &Look| Look {
            mesh: look.mesh.clone(),
            textures: look
                .textures
                .iter()
                .map(|texture| {
                    TextureRef::parse(&texture.path().replacen("_t00_", &number, 1)).unwrap_or_else(|_| texture.clone())
                })
                .collect(),
        };
        Self { front: self.front.as_ref().map(look), back: self.back.as_ref().map(look) }
    }
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

    #[test]
    fn a_hair_color_numbers_the_style_textures() {
        let look = |texture: &str| Look {
            mesh: MeshRef::parse("Fighter.MFighter_m000_m00_ah").unwrap(),
            textures: vec![TextureRef::parse(texture).unwrap()],
        };
        let style = HairStyle { front: Some(look("MFighter.MFighter_m000_t00_m00_ah")), back: None };
        let red = style.colored(2);
        assert_eq!(red.front.unwrap().textures[0].path(), "MFighter.MFighter_m000_t02_m00_ah");
        assert_eq!(style.colored(0), style);
    }
}

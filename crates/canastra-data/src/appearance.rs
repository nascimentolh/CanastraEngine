//! How characters look apart from their gear, and where the lobby stands them.

use serde::{Deserialize, Serialize};

use crate::asset::{MeshRef, TextureRef};
use crate::class::Archetype;
use crate::id::ItemId;
use crate::item::Body;

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

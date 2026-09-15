//! Between a player's client and a game server: admission with the login server's ticket, then the
//! account's characters on this server.

use canastra_data::id::{ClassId, ItemId};
use canastra_data::npc::Race;
use serde::{Deserialize, Serialize};

use crate::ticket::SignedTicket;

/// A character, as stored by its game server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CharacterId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameClient {
    /// First message: the client's protocol version and the ticket the login server issued.
    Hello {
        version: u32,
        ticket: SignedTicket,
    },
    CreateCharacter(NewCharacter),
    DeleteCharacter(CharacterId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameServer {
    /// The ticket admitted the player; the character list follows.
    Admitted,
    /// The client must update; the server closes the connection after this.
    UpdateRequired {
        version: u32,
    },
    /// The server closes the connection after this.
    Refused(Refusal),
    /// The account's characters on this server, after admission and after every change.
    Characters(Vec<CharacterSummary>),
    CreateFailed(CreationFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// Not signed by the login server, or for another game server.
    InvalidTicket,
    Expired,
    /// The ticket was already used; each admits once.
    Reused,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sex {
    Male,
    Female,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Appearance {
    pub hair_style: u8,
    pub hair_color: u8,
    pub face: u8,
}

impl Appearance {
    /// How many of each choice the H5 creation screen offers a character of `race` and `sex`: five male and seven
    /// female hair styles, four hair colors, three for Kamael, whose client textures stop at the third, and three
    /// faces.
    pub fn choices(race: Race, sex: Sex) -> Self {
        let hair_color = if race == Race::Kamael { 3 } else { 4 };
        Self { hair_style: if sex == Sex::Female { 7 } else { 5 }, hair_color, face: 3 }
    }

    /// Whether every choice is one `choices` offers.
    pub fn offered(self, race: Race, sex: Sex) -> bool {
        let offered = Self::choices(race, sex);
        self.hair_style < offered.hair_style && self.hair_color < offered.hair_color && self.face < offered.face
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewCharacter {
    pub name: String,
    /// A starting class.
    pub class: ClassId,
    pub sex: Sex,
    pub appearance: Appearance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub id: CharacterId,
    pub name: String,
    pub class: ClassId,
    pub sex: Sex,
    pub appearance: Appearance,
    pub level: u32,
    /// What the character wears, so the lobby can dress it.
    pub gear: Vec<ItemId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreationFailure {
    /// The name breaks this server's pattern.
    InvalidName,
    /// The name holds a word this server forbids.
    ForbiddenName,
    NameTaken,
    /// The account already has as many characters as this server allows.
    SlotsFull,
    /// Not a class characters start as, or a sex its line does not allow.
    InvalidClass,
    InvalidAppearance,
}

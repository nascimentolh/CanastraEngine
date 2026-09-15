//! The lobby: an admitted player's characters on this server, listed, created and deleted.

use canastra_data::GameData;
use canastra_data::class::{Origin, StartingClass};
use canastra_data::npc::Sex as LineSex;
use canastra_db::{Creation, Database, NewRecord};
use canastra_net::Connection;
use canastra_protocol::game::{CreationFailure, GameClient, GameServer, NewCharacter, Sex};
use canastra_protocol::{AccountId, ServerId};
use tokio::io::{AsyncRead, AsyncWrite};

use super::names::NameRules;
use crate::config::Result;

/// Highest appearance choices the H5 creation screen offers: five male and seven female hair styles, four
/// hair colors and three faces.
const MAX_HAIR_STYLE_MALE: u8 = 4;
const MAX_HAIR_STYLE_FEMALE: u8 = 6;
const MAX_HAIR_COLOR: u8 = 3;
const MAX_FACE: u8 = 2;

pub(crate) struct Lobby {
    pub(crate) server: ServerId,
    pub(crate) database: Database,
    pub(crate) data: GameData,
    pub(crate) names: NameRules,
    /// Characters an account may have on this server.
    pub(crate) slots: u32,
}

impl Lobby {
    /// Sends the account's characters, then answers changes until the player leaves.
    pub(crate) async fn run<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        connection: &mut Connection<S>,
        account: AccountId,
    ) -> Result {
        loop {
            connection.send(&GameServer::Characters(self.database.characters(account, self.server).await?)).await?;
            match connection.recv().await? {
                GameClient::Hello { .. } => return Err("a player said hello twice".into()),
                GameClient::CreateCharacter(new) => {
                    if let Err(failure) = self.create(account, &new).await? {
                        connection.send(&GameServer::CreateFailed(failure)).await?;
                    }
                }
                GameClient::DeleteCharacter(id) => {
                    self.database.delete_character(account, self.server, id).await?;
                }
            }
        }
    }

    async fn create(&self, account: AccountId, new: &NewCharacter) -> Result<std::result::Result<(), CreationFailure>> {
        if let Err(failure) = self.names.check(&new.name) {
            return Ok(Err(failure));
        }
        let position = match spawn_point(&self.data, new, random()?) {
            Ok(position) => position,
            Err(failure) => return Ok(Err(failure)),
        };
        // ponytail: initial items, including the recruit kit, are granted once inventories exist.
        let record = NewRecord {
            account,
            server: self.server,
            name: &new.name,
            class: new.class,
            sex: new.sex,
            appearance: new.appearance,
            position,
        };
        Ok(match self.database.create_character(&record, self.slots).await? {
            Creation::Created(id) => {
                tracing::info!(account = account.0, character = id.0, name = new.name, "character created");
                Ok(())
            }
            Creation::NameTaken => Err(CreationFailure::NameTaken),
            Creation::SlotsFull => Err(CreationFailure::SlotsFull),
        })
    }
}

/// Where `new` appears: one of its starting class's creation points, picked by `roll`. Fails for a class
/// characters do not start as, a sex its line does not allow, or appearance outside the creation screen.
fn spawn_point(data: &GameData, new: &NewCharacter, roll: u32) -> std::result::Result<[i32; 3], CreationFailure> {
    let Some(Origin::Starting(start)) = data.classes.get(&new.class).map(|class| &class.origin) else {
        return Err(CreationFailure::InvalidClass);
    };
    if !allows(start, new.sex) {
        return Err(CreationFailure::InvalidClass);
    }
    let max_hair_style = if new.sex == Sex::Female { MAX_HAIR_STYLE_FEMALE } else { MAX_HAIR_STYLE_MALE };
    let look = new.appearance;
    if look.hair_style > max_hair_style || look.hair_color > MAX_HAIR_COLOR || look.face > MAX_FACE {
        return Err(CreationFailure::InvalidAppearance);
    }
    let points = &start.creation_points;
    let index = usize::try_from(roll).unwrap_or(0) % points.len().max(1);
    points.get(index).copied().ok_or(CreationFailure::InvalidClass)
}

fn allows(start: &StartingClass, sex: Sex) -> bool {
    match start.sex {
        None => true,
        Some(LineSex::Male) => sex == Sex::Male,
        Some(LineSex::Female) => sex == Sex::Female,
        Some(LineSex::Etc) => false,
    }
}

fn random() -> Result<u32> {
    let mut bytes = [0; 4];
    getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use canastra_data::class::PlayerClass;
    use canastra_data::id::ClassId;
    use canastra_data::text::Localized;
    use canastra_protocol::game::Appearance;

    use super::*;

    fn data() -> GameData {
        let class = |id, origin| {
            (ClassId(id), PlayerClass { id: ClassId(id), name: Localized::en("x"), levels: Vec::new(), origin })
        };
        let start = |sex| {
            Origin::Starting(Box::new(StartingClass {
                sex,
                creation_points: vec![[1, 2, 3], [4, 5, 6]],
                ..StartingClass::default()
            }))
        };
        GameData {
            classes: BTreeMap::from([
                class(0, start(None)),
                class(1, Origin::Advanced { parent: ClassId(0) }),
                class(124, start(Some(LineSex::Female))),
            ]),
            ..GameData::default()
        }
    }

    fn new(class: u16, sex: Sex, appearance: Appearance) -> NewCharacter {
        NewCharacter { name: "Ana".into(), class: ClassId(class), sex, appearance }
    }

    #[test]
    fn characters_start_at_their_class_points_within_the_rules() {
        let data = data();
        let plain = Appearance::default();
        assert_eq!(spawn_point(&data, &new(0, Sex::Male, plain), 3), Ok([4, 5, 6]));
        assert_eq!(spawn_point(&data, &new(1, Sex::Male, plain), 0), Err(CreationFailure::InvalidClass));
        assert_eq!(spawn_point(&data, &new(124, Sex::Male, plain), 0), Err(CreationFailure::InvalidClass));
        assert_eq!(spawn_point(&data, &new(124, Sex::Female, plain), 0), Ok([1, 2, 3]));
        let long_hair = Appearance { hair_style: 6, ..plain };
        assert_eq!(spawn_point(&data, &new(0, Sex::Female, long_hair), 0), Ok([1, 2, 3]));
        assert_eq!(spawn_point(&data, &new(0, Sex::Male, long_hair), 0), Err(CreationFailure::InvalidAppearance));
    }
}

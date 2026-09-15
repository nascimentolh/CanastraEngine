//! Characters: each belongs to an account on one game server.

use canastra_data::id::ClassId;
use canastra_protocol::game::{Appearance, CharacterId, CharacterSummary, Sex};
use canastra_protocol::{AccountId, ServerId};

use crate::{Database, Error};

/// A character about to be stored, already validated against the game's rules.
pub struct NewRecord<'a> {
    pub account: AccountId,
    pub server: ServerId,
    pub name: &'a str,
    pub class: ClassId,
    pub sex: Sex,
    pub appearance: Appearance,
    pub position: [i32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Creation {
    Created(CharacterId),
    NameTaken,
    /// The account already has `slots` characters on this server.
    SlotsFull,
}

type Row = (i64, String, i16, bool, i16, i16, i16, i16);

impl Database {
    /// Stores `record` unless its name is taken on its server or the account has no free slot. The account
    /// row is locked meanwhile, so concurrent creations cannot exceed `slots`.
    pub async fn create_character(&self, record: &NewRecord<'_>, slots: u32) -> Result<Creation, Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("SELECT id FROM accounts WHERE id = $1 FOR UPDATE")
            .bind(record.account.0)
            .execute(&mut *transaction)
            .await?;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM characters WHERE account_id = $1 AND server_id = $2")
            .bind(record.account.0)
            .bind(record.server.0.cast_signed())
            .fetch_one(&mut *transaction)
            .await?;
        if count >= i64::from(slots) {
            return Ok(Creation::SlotsFull);
        }
        let [x, y, z] = record.position;
        let id: Option<i64> = sqlx::query_scalar(
            "INSERT INTO characters (account_id, server_id, name, class_id, female, hair_style, hair_color, face, x, y, z) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT DO NOTHING RETURNING id",
        )
        .bind(record.account.0)
        .bind(record.server.0.cast_signed())
        .bind(record.name)
        .bind(record.class.0.cast_signed())
        .bind(record.sex == Sex::Female)
        .bind(i16::from(record.appearance.hair_style))
        .bind(i16::from(record.appearance.hair_color))
        .bind(i16::from(record.appearance.face))
        .bind(x)
        .bind(y)
        .bind(z)
        .fetch_optional(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(id.map_or(Creation::NameTaken, |id| Creation::Created(CharacterId(id))))
    }

    /// The account's characters on `server`, oldest first.
    pub async fn characters(&self, account: AccountId, server: ServerId) -> Result<Vec<CharacterSummary>, Error> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT id, name, class_id, female, hair_style, hair_color, face, level FROM characters \
             WHERE account_id = $1 AND server_id = $2 ORDER BY id",
        )
        .bind(account.0)
        .bind(server.0.cast_signed())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(summary).collect())
    }

    /// Deletes the character if it belongs to `account` on `server`; false when it does not.
    pub async fn delete_character(&self, account: AccountId, server: ServerId, id: CharacterId) -> Result<bool, Error> {
        let result = sqlx::query("DELETE FROM characters WHERE id = $1 AND account_id = $2 AND server_id = $3")
            .bind(id.0)
            .bind(account.0)
            .bind(server.0.cast_signed())
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }
}

fn summary((id, name, class, female, hair_style, hair_color, face, level): Row) -> CharacterSummary {
    // Small columns hold values stored from u8 and u16 fields, so they convert back losslessly.
    let byte = |value: i16| u8::try_from(value).unwrap_or(0);
    CharacterSummary {
        id: CharacterId(id),
        name,
        class: ClassId(class.cast_unsigned()),
        sex: if female { Sex::Female } else { Sex::Male },
        appearance: Appearance { hair_style: byte(hair_style), hair_color: byte(hair_color), face: byte(face) },
        level: u32::try_from(level).unwrap_or(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs PostgreSQL: `CANASTRA_TEST_DATABASE_URL=postgres://... cargo test -p canastra-db -- --ignored`.
    #[tokio::test]
    #[ignore = "needs a PostgreSQL database in CANASTRA_TEST_DATABASE_URL"]
    async fn characters_respect_names_slots_and_ownership() {
        let url = std::env::var("CANASTRA_TEST_DATABASE_URL").expect("CANASTRA_TEST_DATABASE_URL is set");
        let database = Database::connect(&url).await.unwrap();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
        let account = database.create_account(&format!("chars_{nanos}"), "secret").await.unwrap();
        let other = database.create_account(&format!("other_{nanos}"), "secret").await.unwrap();
        let server = ServerId(900);
        let name = format!("Hero{nanos}");
        let record = |account, name| NewRecord {
            account,
            server,
            name,
            class: ClassId(0),
            sex: Sex::Male,
            appearance: Appearance::default(),
            position: [1, 2, 3],
        };

        let Creation::Created(id) = database.create_character(&record(account, &name), 2).await.unwrap() else {
            panic!("the first character is created")
        };
        let upper = name.to_uppercase();
        assert_eq!(database.create_character(&record(other, &upper), 2).await.unwrap(), Creation::NameTaken);
        let second = format!("Second{nanos}");
        assert!(matches!(database.create_character(&record(account, &second), 2).await.unwrap(), Creation::Created(_)));
        let third = format!("Third{nanos}");
        assert_eq!(database.create_character(&record(account, &third), 2).await.unwrap(), Creation::SlotsFull);

        let names: Vec<_> = database.characters(account, server).await.unwrap().into_iter().map(|c| c.name).collect();
        assert_eq!(names, [name, second]);
        assert!(!database.delete_character(other, server, id).await.unwrap());
        assert!(database.delete_character(account, server, id).await.unwrap());
        assert_eq!(database.characters(account, server).await.unwrap().len(), 1);
    }
}

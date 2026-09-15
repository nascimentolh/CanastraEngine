//! Accounts: created by server tools, authenticated by the login server.

use std::fmt;
use std::sync::OnceLock;

use canastra_protocol::AccountId;

use crate::{Database, Error, password};

/// The outcome of checking an account name and password.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authentication {
    Accepted(AccountId),
    /// Unknown account or wrong password, not told apart.
    WrongCredentials,
    Banned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Length,
    /// Only ASCII letters, digits and underscores are allowed.
    Characters,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Length => "account names are 3 to 32 characters",
            Self::Characters => "account names use only letters, digits and underscores",
        })
    }
}

/// Checks an account name: 3 to 32 ASCII letters, digits or underscores.
pub fn validate_name(name: &str) -> Result<(), NameError> {
    if !(3..=32).contains(&name.len()) {
        Err(NameError::Length)
    } else if !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
        Err(NameError::Characters)
    } else {
        Ok(())
    }
}

impl Database {
    pub async fn create_account(&self, name: &str, password: &str) -> Result<AccountId, Error> {
        validate_name(name).map_err(Error::InvalidName)?;
        let hash = password::hash(password.to_owned()).await?;
        let inserted = sqlx::query_scalar::<_, i64>(
            "INSERT INTO accounts (name, password_hash) VALUES ($1, $2) ON CONFLICT DO NOTHING RETURNING id",
        )
        .bind(name)
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;
        inserted.map(AccountId).ok_or(Error::NameTaken)
    }

    /// Checks `password` for the account `name`, recording the login when it is accepted. Unknown names
    /// cost a hash check too, so response time does not reveal which accounts exist.
    pub async fn authenticate(&self, name: &str, password: &str) -> Result<Authentication, Error> {
        let row = sqlx::query_as::<_, (i64, String, bool)>(
            "SELECT id, password_hash, banned FROM accounts WHERE lower(name) = lower($1)",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        let Some((id, stored, banned)) = row else {
            password::verify(password.to_owned(), decoy_hash().await?).await?;
            return Ok(Authentication::WrongCredentials);
        };
        if !password::verify(password.to_owned(), stored).await? {
            return Ok(Authentication::WrongCredentials);
        }
        if banned {
            return Ok(Authentication::Banned);
        }
        sqlx::query("UPDATE accounts SET last_login_at = now() WHERE id = $1").bind(id).execute(&self.pool).await?;
        Ok(Authentication::Accepted(AccountId(id)))
    }
}

/// A hash no password is expected to match, computed once.
async fn decoy_hash() -> Result<String, Error> {
    static DECOY: OnceLock<String> = OnceLock::new();
    if let Some(hash) = DECOY.get() {
        return Ok(hash.clone());
    }
    let hash = password::hash("no account has this password".to_owned()).await?;
    Ok(DECOY.get_or_init(|| hash).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_short_ascii_words() {
        assert_eq!(validate_name("ana_2"), Ok(()));
        assert_eq!(validate_name("an"), Err(NameError::Length));
        assert_eq!(validate_name("ana maria"), Err(NameError::Characters));
        assert_eq!(validate_name("anã"), Err(NameError::Characters));
    }

    /// Needs PostgreSQL: `CANASTRA_TEST_DATABASE_URL=postgres://... cargo test -p canastra-db -- --ignored`.
    #[tokio::test]
    #[ignore = "needs a PostgreSQL database in CANASTRA_TEST_DATABASE_URL"]
    async fn accounts_authenticate_by_name_regardless_of_case() {
        let url = std::env::var("CANASTRA_TEST_DATABASE_URL").expect("CANASTRA_TEST_DATABASE_URL is set");
        let database = Database::connect(&url).await.unwrap();
        let name = format!("test_{}", std::process::id());
        let id = database.create_account(&name, "secret").await.unwrap();

        assert!(matches!(database.create_account(&name.to_uppercase(), "other").await, Err(Error::NameTaken)));
        assert_eq!(database.authenticate(&name.to_uppercase(), "secret").await.unwrap(), Authentication::Accepted(id));
        assert_eq!(database.authenticate(&name, "wrong").await.unwrap(), Authentication::WrongCredentials);
        assert_eq!(database.authenticate("nobody_here", "secret").await.unwrap(), Authentication::WrongCredentials);

        sqlx::query("UPDATE accounts SET banned = TRUE WHERE id = $1")
            .bind(id.0)
            .execute(&database.pool)
            .await
            .unwrap();
        assert_eq!(database.authenticate(&name, "secret").await.unwrap(), Authentication::Banned);
        sqlx::query("DELETE FROM accounts WHERE id = $1").bind(id.0).execute(&database.pool).await.unwrap();
    }
}

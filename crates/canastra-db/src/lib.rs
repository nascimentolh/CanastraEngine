//! PostgreSQL storage for Canastra servers. Connecting applies the embedded migrations, so a database
//! is always at the schema this build expects.

mod accounts;
mod characters;

use std::fmt;

pub use accounts::{Authentication, NameError, validate_name};
pub use characters::{Creation, NewRecord};

pub struct Database {
    pool: sqlx::PgPool,
}

impl Database {
    /// Connects to the database at `url` and brings its schema up to date.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let pool = sqlx::PgPool::connect(url).await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }
}

#[derive(Debug)]
pub enum Error {
    Database(sqlx::Error),
    Migration(sqlx::migrate::MigrateError),
    /// Hashing a password failed or a stored hash is malformed.
    Password(String),
    InvalidName(NameError),
    NameTaken,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "database: {error}"),
            Self::Migration(error) => write!(f, "database migration: {error}"),
            Self::Password(error) => write!(f, "password hashing: {error}"),
            Self::InvalidName(error) => write!(f, "{error}"),
            Self::NameTaken => f.write_str("that account name is taken"),
        }
    }
}

impl std::error::Error for Error {}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<sqlx::migrate::MigrateError> for Error {
    fn from(error: sqlx::migrate::MigrateError) -> Self {
        Self::Migration(error)
    }
}

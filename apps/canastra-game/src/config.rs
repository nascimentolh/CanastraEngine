//! The game server's TOML configuration. `CANASTRA_DATABASE_URL`, `CANASTRA_NOISE_PRIVATE` and
//! `CANASTRA_NOISE_PUBLIC` override the file, as containers pass secrets.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use canastra_net::Keypair;
use serde::Deserialize;

pub(crate) type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// This server's id, as the login server authorizes it.
    pub(crate) id: u16,
    pub(crate) name: String,
    /// Where this server listens for players.
    pub(crate) players: SocketAddr,
    /// Where players reach this server, as `host:port`.
    pub(crate) public_address: String,
    pub(crate) capacity: u32,
    #[serde(default)]
    pub(crate) database_url: String,
    /// The game data file (`.cana`) this server plays by.
    pub(crate) game_data: PathBuf,
    pub(crate) login: Login,
    #[serde(default)]
    pub(crate) characters: Characters,
    #[serde(default)]
    pub(crate) keys: Keys,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Characters {
    /// Characters an account may have on this server.
    pub(crate) slots: u32,
    /// A regular expression the whole name must match.
    pub(crate) name_pattern: String,
    /// Words names may not contain, ignoring case.
    pub(crate) forbidden_names: Vec<String>,
}

impl Default for Characters {
    fn default() -> Self {
        Self { slots: 7, name_pattern: "[A-Za-z0-9]{2,16}".into(), forbidden_names: Vec::new() }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Login {
    /// The login server's internal registration address.
    pub(crate) address: SocketAddr,
    /// The login server's `noise_public` key.
    pub(crate) public_key: String,
    /// The login server's public ticket key, printed by its keygen.
    pub(crate) ticket_public: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Keys {
    #[serde(default)]
    pub(crate) noise_private: String,
    #[serde(default)]
    pub(crate) noise_public: String,
}

impl Config {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut config: Self = toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        for (name, field) in [
            ("CANASTRA_DATABASE_URL", &mut config.database_url),
            ("CANASTRA_NOISE_PRIVATE", &mut config.keys.noise_private),
            ("CANASTRA_NOISE_PUBLIC", &mut config.keys.noise_public),
        ] {
            if let Ok(value) = std::env::var(name) {
                *field = value;
            }
        }
        if config.database_url.is_empty() {
            return Err("database_url is not set".into());
        }
        Ok(config)
    }

    pub(crate) fn keypair(&self) -> Result<Keypair> {
        Ok(Keypair::from_hex(&self.keys.noise_private, &self.keys.noise_public)
            .map_err(|error| format!("keys.noise_private / keys.noise_public: {error}"))?)
    }

    pub(crate) fn tickets(&self) -> Result<canastra_net::ticket::Checker> {
        Ok(canastra_net::ticket::Checker::from_hex(&self.login.ticket_public)
            .map_err(|error| format!("login.ticket_public: {error}"))?)
    }

    pub(crate) fn login_key(&self) -> Result<[u8; 32]> {
        Ok(canastra_net::parse_key(&self.login.public_key).map_err(|error| format!("login.public_key: {error}"))?)
    }
}

/// A fresh `[keys]` section; the public key goes into the login server's `[[authorized]]` list.
pub(crate) fn keygen() -> Result<String> {
    let keys = Keypair::generate()?;
    Ok(format!(
        "[keys]\nnoise_private = \"{}\"\n# Authorize this key on the login server.\nnoise_public = \"{}\"\n",
        hex::encode(keys.private),
        hex::encode(keys.public)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keygen_output_loads_as_keys() {
        let text = format!(
            "id = 1\nname = \"Canastra\"\nplayers = \"0.0.0.0:7777\"\npublic_address = \"127.0.0.1:7777\"\n\
             capacity = 100\ngame_data = \"gamedata.cana\"\n[login]\naddress = \"127.0.0.1:2107\"\npublic_key = \"{}\"\nticket_public = \"{}\"\n{}",
            "00".repeat(32),
            canastra_net::ticket::Issuer::generate().unwrap().public_hex(),
            keygen().unwrap()
        );
        let config: Config = toml::from_str(&text).unwrap();
        assert!(config.keypair().is_ok() && config.login_key().is_ok() && config.tickets().is_ok());
    }

    #[test]
    fn the_example_configuration_parses() {
        let config: Config = toml::from_str(include_str!("../canastra-game.example.toml")).unwrap();
        assert_eq!(config.characters.slots, 7);
    }
}

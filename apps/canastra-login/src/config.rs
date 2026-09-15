//! The login server's TOML configuration. Secrets may come from the environment instead, as containers
//! pass them: `CANASTRA_DATABASE_URL`, `CANASTRA_NOISE_PRIVATE`, `CANASTRA_NOISE_PUBLIC` and
//! `CANASTRA_TICKET_SECRET` override the file.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;

use canastra_net::Keypair;
use canastra_net::ticket::Issuer;
use canastra_protocol::ServerId;
use serde::Deserialize;

pub(crate) type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    #[serde(default)]
    pub(crate) database_url: String,
    /// Where players connect.
    pub(crate) players: SocketAddr,
    /// Where game servers register; keep it off the public network.
    pub(crate) game_servers: SocketAddr,
    /// How long a ticket admits its player.
    #[serde(default = "default_ticket_seconds")]
    pub(crate) ticket_seconds: u64,
    #[serde(default)]
    pub(crate) keys: KeysConfig,
    /// Game servers allowed to register, by their public key.
    #[serde(default)]
    pub(crate) authorized: Vec<Authorized>,
    #[serde(default)]
    pub(crate) limits: Limits,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeysConfig {
    #[serde(default)]
    pub(crate) noise_private: String,
    #[serde(default)]
    pub(crate) noise_public: String,
    #[serde(default)]
    pub(crate) ticket_secret: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Authorized {
    pub(crate) id: u16,
    pub(crate) public_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Limits {
    /// Login attempts allowed per address and per account within the window.
    pub(crate) attempts: usize,
    pub(crate) window_seconds: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { attempts: 5, window_seconds: 60 }
    }
}

fn default_ticket_seconds() -> u64 {
    30
}

/// The keys a running login server uses.
pub(crate) struct Keys {
    pub(crate) noise: Keypair,
    pub(crate) tickets: Issuer,
    pub(crate) authorized: HashMap<[u8; 32], ServerId>,
}

impl Config {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut config: Self = toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        let overrides = [
            ("CANASTRA_DATABASE_URL", &mut config.database_url),
            ("CANASTRA_NOISE_PRIVATE", &mut config.keys.noise_private),
            ("CANASTRA_NOISE_PUBLIC", &mut config.keys.noise_public),
            ("CANASTRA_TICKET_SECRET", &mut config.keys.ticket_secret),
        ];
        for (name, field) in overrides {
            if let Ok(value) = std::env::var(name) {
                *field = value;
            }
        }
        if config.database_url.is_empty() {
            return Err("database_url is not set".into());
        }
        Ok(config)
    }

    pub(crate) fn keys(&self) -> Result<Keys> {
        let noise = Keypair::from_hex(&self.keys.noise_private, &self.keys.noise_public)
            .map_err(|error| format!("keys.noise_private / keys.noise_public: {error}"))?;
        let tickets =
            Issuer::from_hex(&self.keys.ticket_secret).map_err(|error| format!("keys.ticket_secret: {error}"))?;
        let authorized = self
            .authorized
            .iter()
            .map(|server| {
                canastra_net::parse_key(&server.public_key)
                    .map(|key| (key, ServerId(server.id)))
                    .map_err(|error| format!("authorized server {}: {error}", server.id))
            })
            .collect::<std::result::Result<_, _>>()?;
        Ok(Keys { noise, tickets, authorized })
    }
}

/// A fresh `[keys]` section for a new login server.
pub(crate) fn keygen() -> Result<String> {
    let noise = Keypair::generate()?;
    let tickets = Issuer::generate()?;
    Ok(format!(
        "[keys]\n\
         noise_private = \"{}\"\n\
         # Clients pin this key.\n\
         noise_public = \"{}\"\n\
         ticket_secret = \"{}\"\n\
         # Game servers check tickets with ticket_public = \"{}\"\n",
        hex::encode(noise.private),
        hex::encode(noise.public),
        tickets.secret_hex(),
        tickets.public_hex(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keygen_output_loads_as_keys() {
        let text = format!(
            "database_url = \"postgres://x\"\nplayers = \"127.0.0.1:2106\"\ngame_servers = \"127.0.0.1:2107\"\n{}",
            keygen().unwrap()
        );
        let config: Config = toml::from_str(&text).unwrap();
        assert_eq!(config.ticket_seconds, 30);
        assert!(config.keys().is_ok());
    }
}

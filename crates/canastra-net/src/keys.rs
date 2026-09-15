//! Static Noise keys: Curve25519 key pairs, written as hex in configuration files.

use crate::Error;
use crate::connection::Pattern;

#[derive(Clone, PartialEq, Eq)]
pub struct Keypair {
    pub private: [u8; 32],
    pub public: [u8; 32],
}

impl Keypair {
    /// A new random key pair.
    pub fn generate() -> Result<Self, Error> {
        let keys = snow::Builder::new(Pattern::Player.params()).generate_keypair()?;
        Ok(Self { private: key(&keys.private)?, public: key(&keys.public)? })
    }

    /// A key pair from its two keys as hex; a mismatched pair fails at the handshake.
    pub fn from_hex(private: &str, public: &str) -> Result<Self, Error> {
        Ok(Self { private: parse_key(private)?, public: parse_key(public)? })
    }
}

impl std::fmt::Debug for Keypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The private key never reaches logs.
        write!(f, "Keypair {{ public: {} }}", hex::encode(self.public))
    }
}

/// A 32-byte key from 64 hex digits.
pub fn parse_key(text: &str) -> Result<[u8; 32], Error> {
    key(&hex::decode(text.trim()).map_err(|_| Error::BadKey)?)
}

fn key(bytes: &[u8]) -> Result<[u8; 32], Error> {
    bytes.try_into().map_err(|_| Error::BadKey)
}

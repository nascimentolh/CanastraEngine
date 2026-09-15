//! The login server's admission ticket: short-lived, single use, for one account on one game server,
//! signed so game servers check it without asking the login server.

use serde::{Deserialize, Serialize};

use crate::{AccountId, ServerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    pub account: AccountId,
    pub server: ServerId,
    /// Unix time in seconds after which the ticket is refused.
    pub expires_at: u64,
    /// Random, so each ticket is used once.
    pub nonce: [u8; 16],
}

/// A ticket's encoding and the login server's Ed25519 signature over it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedTicket {
    pub ticket: Vec<u8>,
    pub signature: Vec<u8>,
}

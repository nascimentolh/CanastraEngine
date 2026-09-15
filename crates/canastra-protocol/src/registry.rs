//! Between a game server and the login server, over the internal channel: registration and live
//! population, so the login server lists only servers that are online.

use serde::{Deserialize, Serialize};

use crate::ServerId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameToLogin {
    /// First message: the game server's protocol version and what players see of it.
    Register {
        version: u32,
        id: ServerId,
        name: String,
        address: String,
        capacity: u32,
    },
    Population(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoginToGame {
    Registered,
    /// Registration failed; the login server closes the channel after this.
    Rejected(Rejection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    UpdateRequired {
        version: u32,
    },
    /// The id is not the one authorized for this server's key.
    WrongId,
    /// Another connection already holds this id.
    AlreadyRegistered,
}

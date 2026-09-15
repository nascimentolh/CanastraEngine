//! Between a player's client and a game server: admission with the login server's ticket.

use serde::{Deserialize, Serialize};

use crate::ticket::SignedTicket;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameClient {
    /// First message: the client's protocol version and the ticket the login server issued.
    Hello { version: u32, ticket: SignedTicket },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameServer {
    /// The ticket admitted the player.
    Admitted,
    /// The client must update; the server closes the connection after this.
    UpdateRequired { version: u32 },
    /// The server closes the connection after this.
    Refused(Refusal),
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

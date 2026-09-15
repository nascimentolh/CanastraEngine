//! Between a player's client and the login server: version check, authentication, the game server
//! list and the ticket that admits the player to one game server.

use serde::{Deserialize, Serialize};

use crate::ServerId;
use crate::ticket::SignedTicket;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoginClient {
    /// First message: the client's protocol version.
    Hello {
        version: u32,
    },
    Authenticate {
        account: String,
        password: String,
    },
    /// Asks to play on one of the listed servers.
    RequestTicket {
        server: ServerId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoginServer {
    /// The versions match; authentication may start.
    Welcome,
    /// The client must update; the server closes the connection after this.
    UpdateRequired {
        version: u32,
    },
    AuthFailed(AuthFailure),
    /// Authentication succeeded: the game servers online now.
    Servers(Vec<ServerEntry>),
    Ticket(SignedTicket),
    TicketRefused(TicketRefusal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthFailure {
    /// Unknown account or wrong password, deliberately not told apart.
    WrongCredentials,
    Banned,
    /// Too many attempts from this address or for this account; retry later.
    TooManyAttempts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TicketRefusal {
    /// The server is not online anymore.
    Offline,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerEntry {
    pub id: ServerId,
    pub name: String,
    /// Where clients connect, as `host:port`.
    pub address: String,
    pub population: u32,
    pub capacity: u32,
}

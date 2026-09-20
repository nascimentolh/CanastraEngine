//! The client's network thread: a tokio runtime that talks to the login server, then to the chosen game
//! server, and hands replies back to the window's event loop.

mod session;

use std::net::SocketAddr;

use canastra_protocol::game::{CharacterId, CharacterSummary, CreationFailure, InWorld, NewCharacter, Refusal};
use canastra_protocol::login::{AuthFailure, ServerEntry, TicketRefusal};
use tokio::sync::mpsc;
use winit::event_loop::EventLoopProxy;

use session::Session;

/// What the game loop asks of the network.
pub(crate) enum Request {
    Login {
        account: String,
        password: String,
    },
    /// Asks the login server for a ticket to `server`, then joins it.
    Join(ServerEntry),
    Create(NewCharacter),
    Delete(CharacterId),
    /// Enters the world with a character of the joined server.
    Enter(CharacterId),
}

/// What the network reports back, as window events.
#[derive(Debug)]
pub(crate) enum Reply {
    /// The connection failed or dropped; a new login starts over.
    Failed(String),
    UpdateRequired,
    AuthFailed(AuthFailure),
    Servers(Vec<ServerEntry>),
    TicketRefused(TicketRefusal),
    GameRefused(Refusal),
    /// The account's characters on the joined server, after joining or a change; `failure` says why a
    /// creation just failed.
    Characters {
        list: Vec<CharacterSummary>,
        failure: Option<CreationFailure>,
    },
    /// The player is in the world, with the character it entered as.
    Entered(InWorld),
}

/// Where the login server is and the key it must prove it holds.
pub(crate) struct LoginAddress {
    pub(crate) address: SocketAddr,
    pub(crate) key: [u8; 32],
}

impl LoginAddress {
    /// From `CANASTRA_LOGIN` (default `127.0.0.1:2106`) and `CANASTRA_LOGIN_KEY`, the login server's
    /// `noise_public` key.
    // ponytail: read from the environment until the launcher hands the client its server and pinned key.
    pub(crate) fn from_env() -> Result<Self, String> {
        let address = std::env::var("CANASTRA_LOGIN").unwrap_or_else(|_| "127.0.0.1:2106".into());
        let address = address.parse().map_err(|_| format!("CANASTRA_LOGIN `{address}` is not host:port"))?;
        let key = std::env::var("CANASTRA_LOGIN_KEY").map_err(|_| "CANASTRA_LOGIN_KEY is not set".to_owned())?;
        let key = canastra_net::parse_key(&key).map_err(|error| format!("CANASTRA_LOGIN_KEY: {error}"))?;
        Ok(Self { address, key })
    }
}

pub(crate) struct Network {
    requests: mpsc::UnboundedSender<Request>,
}

impl Network {
    /// Starts the network thread; replies wake the event loop through `proxy`.
    pub(crate) fn start(server: LoginAddress, proxy: EventLoopProxy<crate::app::Event>) -> Result<Self, String> {
        let (requests, receiver) = mpsc::unbounded_channel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("network runtime: {error}"))?;
        std::thread::Builder::new()
            .name("network".into())
            .spawn(move || runtime.block_on(run(server, receiver, proxy)))
            .map_err(|error| format!("network thread: {error}"))?;
        Ok(Self { requests })
    }

    pub(crate) fn send(&self, request: Request) {
        // The thread only stops with the process, so the channel stays open.
        let _ = self.requests.send(request);
    }
}

async fn run(
    server: LoginAddress,
    mut requests: mpsc::UnboundedReceiver<Request>,
    proxy: EventLoopProxy<crate::app::Event>,
) {
    let mut session = Session::default();
    while let Some(request) = requests.recv().await {
        let reply = match session.handle(&server, request).await {
            Ok(reply) => reply,
            Err(error) => {
                session = Session::default();
                Reply::Failed(error.to_string())
            }
        };
        if proxy.send_event(crate::app::Event::Network(reply)).is_err() {
            return;
        }
    }
}

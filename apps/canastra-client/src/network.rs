//! The client's network thread: a tokio runtime that talks to the login server and hands replies back to
//! the window's event loop.

use std::net::SocketAddr;

use canastra_net::Connection;
use canastra_protocol::login::{AuthFailure, LoginClient, LoginServer, ServerEntry, TicketRefusal};
use canastra_protocol::ticket::SignedTicket;
use canastra_protocol::{ServerId, VERSION};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use winit::event_loop::EventLoopProxy;

/// What the game loop asks of the network.
pub(crate) enum Request {
    Login { account: String, password: String },
    Ticket(ServerId),
}

/// What the network reports back, as window events.
#[derive(Debug)]
pub(crate) enum Reply {
    /// The connection failed or dropped; a new login starts over.
    Failed(String),
    UpdateRequired,
    AuthFailed(AuthFailure),
    Servers(Vec<ServerEntry>),
    Ticket(ServerId, SignedTicket),
    TicketRefused(TicketRefusal),
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
    pub(crate) fn start(server: LoginAddress, proxy: EventLoopProxy<Reply>) -> Result<Self, String> {
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

async fn run(server: LoginAddress, mut requests: mpsc::UnboundedReceiver<Request>, proxy: EventLoopProxy<Reply>) {
    let mut connection = None;
    while let Some(request) = requests.recv().await {
        let reply = match handle(&server, &mut connection, request).await {
            Ok(reply) => reply,
            Err(error) => {
                connection = None;
                Reply::Failed(error)
            }
        };
        if proxy.send_event(reply).is_err() {
            return;
        }
    }
}

async fn handle(
    server: &LoginAddress,
    connection: &mut Option<Connection<TcpStream>>,
    request: Request,
) -> Result<Reply, String> {
    let error = |error: canastra_net::Error| error.to_string();
    match request {
        Request::Login { account, password } => {
            // Each login starts on a fresh connection, as the server expects authentication right after hello.
            let stream = TcpStream::connect(server.address).await.map_err(|error| error.to_string())?;
            let mut fresh = Connection::connect(stream, &server.key, None).await.map_err(error)?;
            fresh.send(&LoginClient::Hello { version: VERSION }).await.map_err(error)?;
            match fresh.recv::<LoginServer>().await.map_err(error)? {
                LoginServer::Welcome => {}
                LoginServer::UpdateRequired { .. } => return Ok(Reply::UpdateRequired),
                other => return Err(format!("unexpected reply to hello: {other:?}")),
            }
            fresh.send(&LoginClient::Authenticate { account, password }).await.map_err(error)?;
            let reply = match fresh.recv::<LoginServer>().await.map_err(error)? {
                LoginServer::AuthFailed(failure) => Reply::AuthFailed(failure),
                LoginServer::Servers(servers) => Reply::Servers(servers),
                other => return Err(format!("unexpected reply to authentication: {other:?}")),
            };
            *connection = Some(fresh);
            Ok(reply)
        }
        Request::Ticket(id) => {
            let current = connection.as_mut().ok_or("not logged in")?;
            current.send(&LoginClient::RequestTicket { server: id }).await.map_err(error)?;
            match current.recv::<LoginServer>().await.map_err(error)? {
                LoginServer::Ticket(ticket) => Ok(Reply::Ticket(id, ticket)),
                LoginServer::TicketRefused(refusal) => Ok(Reply::TicketRefused(refusal)),
                other => Err(format!("unexpected reply to a ticket request: {other:?}")),
            }
        }
    }
}

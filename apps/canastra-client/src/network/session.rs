//! The connections a player holds, and how each request travels over them.

use canastra_net::Connection;
use canastra_protocol::VERSION;
use canastra_protocol::game::{GameClient, GameServer};
use canastra_protocol::login::{LoginClient, LoginServer};
use tokio::net::TcpStream;

use super::{LoginAddress, Reply, Request};

/// To the login server after authenticating, then to a game server after joining.
#[derive(Default)]
pub(super) struct Session {
    login: Option<Connection<TcpStream>>,
    game: Option<Connection<TcpStream>>,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl Session {
    pub(super) async fn handle(&mut self, server: &LoginAddress, request: Request) -> Result<Reply> {
        match request {
            Request::Login { account, password } => self.login(server, account, password).await,
            Request::Join(entry) => {
                let login = self.login.as_mut().ok_or("not logged in")?;
                login.send(&LoginClient::RequestTicket { server: entry.id }).await?;
                let ticket = match login.recv::<LoginServer>().await? {
                    LoginServer::Ticket(ticket) => ticket,
                    LoginServer::TicketRefused(refusal) => return Ok(Reply::TicketRefused(refusal)),
                    other => return Err(format!("unexpected reply to a ticket request: {other:?}").into()),
                };
                let stream = TcpStream::connect(entry.address.as_str()).await?;
                let mut game = Connection::connect(stream, &entry.key, None).await?;
                game.send(&GameClient::Hello { version: VERSION, ticket }).await?;
                match game.recv::<GameServer>().await? {
                    GameServer::Admitted => {}
                    GameServer::UpdateRequired { .. } => return Ok(Reply::UpdateRequired),
                    GameServer::Refused(refusal) => return Ok(Reply::GameRefused(refusal)),
                    other => return Err(format!("unexpected reply to hello: {other:?}").into()),
                }
                let game = self.game.insert(game);
                characters(game).await
            }
            Request::Create(new) => self.change(GameClient::CreateCharacter(new)).await,
            Request::Delete(id) => self.change(GameClient::DeleteCharacter(id)).await,
            Request::Move(to) => {
                let game = self.game.as_mut().ok_or("not on a game server")?;
                game.send(&GameClient::MoveTo(to)).await?;
                match game.recv::<GameServer>().await? {
                    GameServer::Moving(walk) => Ok(Reply::Moving(walk)),
                    other => Err(format!("unexpected reply to a walk: {other:?}").into()),
                }
            }
            Request::Enter(id) => {
                let game = self.game.as_mut().ok_or("not on a game server")?;
                game.send(&GameClient::EnterWorld(id)).await?;
                match game.recv::<GameServer>().await? {
                    GameServer::Entered(world) => Ok(Reply::Entered(world)),
                    other => Err(format!("unexpected reply to entering the world: {other:?}").into()),
                }
            }
        }
    }

    /// Sends a change to the joined server's characters and waits for the list after it.
    async fn change(&mut self, message: GameClient) -> Result<Reply> {
        let game = self.game.as_mut().ok_or("not on a game server")?;
        game.send(&message).await?;
        characters(game).await
    }

    async fn login(&mut self, server: &LoginAddress, account: String, password: String) -> Result<Reply> {
        // Each login starts over on a fresh connection, as the server expects authentication right after hello.
        *self = Self::default();
        let stream = TcpStream::connect(server.address).await?;
        let mut login = Connection::connect(stream, &server.key, None).await?;
        login.send(&LoginClient::Hello { version: VERSION }).await?;
        match login.recv::<LoginServer>().await? {
            LoginServer::Welcome => {}
            LoginServer::UpdateRequired { .. } => return Ok(Reply::UpdateRequired),
            other => return Err(format!("unexpected reply to hello: {other:?}").into()),
        }
        login.send(&LoginClient::Authenticate { account, password }).await?;
        let reply = match login.recv::<LoginServer>().await? {
            LoginServer::AuthFailed(failure) => Reply::AuthFailed(failure),
            LoginServer::Servers(servers) => Reply::Servers(servers),
            other => return Err(format!("unexpected reply to authentication: {other:?}").into()),
        };
        self.login = Some(login);
        Ok(reply)
    }
}

/// Waits for the character list the game server sends after admission and after each change, noting a
/// creation failure that comes before it.
async fn characters(game: &mut Connection<TcpStream>) -> Result<Reply> {
    let mut failure = None;
    loop {
        match game.recv::<GameServer>().await? {
            GameServer::Characters(list) => return Ok(Reply::Characters { list, failure }),
            GameServer::CreateFailed(reason) => failure = Some(reason),
            other => return Err(format!("unexpected message in the lobby: {other:?}").into()),
        }
    }
}

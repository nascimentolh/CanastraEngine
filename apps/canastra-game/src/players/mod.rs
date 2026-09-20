//! Player connections: admission by the login server's ticket, then the lobby of the account's characters.

mod admission;
mod lobby;
mod names;

use std::sync::Arc;

use canastra_net::{Connection, Keypair, Pattern};
use canastra_protocol::AccountId;
use canastra_protocol::game::GameClient;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;

pub(crate) use admission::Admission;
pub(crate) use lobby::Lobby;
pub(crate) use names::NameRules;

use crate::config::Result;

pub(crate) struct Players {
    pub(crate) keys: Keypair,
    pub(crate) admission: Admission,
    pub(crate) lobby: Lobby,
}

pub(crate) async fn listen(listener: TcpListener, players: Arc<Players>) {
    loop {
        let Ok((stream, address)) = listener.accept().await else { continue };
        let players = players.clone();
        tokio::spawn(async move {
            if let Err(error) = serve(stream, &players).await {
                tracing::debug!(%address, %error, "player connection closed");
            }
        });
    }
}

async fn serve<S: AsyncRead + AsyncWrite + Unpin>(stream: S, players: &Players) -> Result {
    let mut connection = Connection::accept(stream, &players.keys, Pattern::Player).await?;
    let Some(account) = players.admission.admit(&mut connection).await? else { return Ok(()) };
    let result = play(&mut connection, players, account).await;
    players.admission.release();
    result
}

/// The lobby, and then the world for as long as the player stays in it.
async fn play<S: AsyncRead + AsyncWrite + Unpin>(
    connection: &mut Connection<S>,
    players: &Players,
    account: AccountId,
) -> Result {
    let entered = players.lobby.run(connection, account).await?;
    let character = &entered.character;
    tracing::info!(account = account.0, character = character.id.0, name = character.name, "player in the world");
    // ponytail: the world takes no orders yet, so the player only holds its place until it disconnects.
    let message = connection.recv::<GameClient>().await?;
    Err(format!("a player in the world sent {message:?}").into())
}

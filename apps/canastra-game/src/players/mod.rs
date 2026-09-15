//! Player connections: admission by the login server's ticket, then the lobby of the account's characters.

mod admission;
mod lobby;
mod names;

use std::sync::Arc;

use canastra_net::{Connection, Keypair, Pattern};
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
    let result = players.lobby.run(&mut connection, account).await;
    players.admission.release();
    result
}

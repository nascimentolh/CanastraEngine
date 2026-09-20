//! Player connections: admission by the login server's ticket, then the lobby of the account's characters.

mod admission;
mod lobby;
mod names;
mod registry;
mod world;

use std::sync::Arc;

use canastra_net::{Connection, Keypair, Pattern};
use canastra_protocol::AccountId;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;

pub(crate) use admission::Admission;
pub(crate) use lobby::Lobby;
pub(crate) use names::NameRules;
pub(crate) use registry::Registry;

use crate::config::Result;

pub(crate) struct Players {
    pub(crate) keys: Keypair,
    pub(crate) admission: Admission,
    pub(crate) lobby: Lobby,
    /// The ground the world stands on, when this server has geodata.
    pub(crate) geo: Option<crate::geo::Geo>,
    /// The players in the world and how to reach them.
    pub(crate) world: Registry,
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

async fn serve<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(stream: S, players: &Players) -> Result {
    let mut connection = Connection::accept(stream, &players.keys, Pattern::Player).await?;
    let Some(account) = players.admission.admit(&mut connection).await? else { return Ok(()) };
    let result = play(connection, players, account).await;
    players.admission.release();
    result
}

/// The lobby, and then the world for as long as the player stays in it.
async fn play<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    mut connection: Connection<S>,
    players: &Players,
    account: AccountId,
) -> Result {
    let entered = players.lobby.run(&mut connection, account, players.geo.as_ref()).await?;
    let character = &entered.character;
    let inside = players.world.len() + 1;
    tracing::info!(
        account = account.0,
        character = character.id.0,
        name = character.name,
        inside,
        "player in the world"
    );
    world::run(connection, players, entered).await
}

//! Game server registration over the internal channel: only authorized keys register, each under its
//! own id, and a server leaves the list when its channel closes.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use canastra_net::{Connection, Keypair, Pattern};
use canastra_protocol::login::ServerEntry;
use canastra_protocol::registry::{GameToLogin, LoginToGame, Rejection};
use canastra_protocol::{ServerId, VERSION};
use tokio::net::{TcpListener, TcpStream};

use crate::config::Result;

/// The game servers online now.
#[derive(Clone, Default)]
pub(crate) struct Servers(Arc<Mutex<BTreeMap<ServerId, ServerEntry>>>);

impl Servers {
    pub(crate) fn list(&self) -> Vec<ServerEntry> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).values().cloned().collect()
    }

    pub(crate) fn get(&self, id: ServerId) -> Option<ServerEntry> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).get(&id).cloned()
    }

    /// Adds `entry` unless its id is already online.
    fn insert(&self, entry: ServerEntry) -> bool {
        let mut servers = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if servers.contains_key(&entry.id) {
            return false;
        }
        servers.insert(entry.id, entry);
        true
    }

    fn update(&self, id: ServerId, population: u32) {
        if let Some(entry) = self.0.lock().unwrap_or_else(PoisonError::into_inner).get_mut(&id) {
            entry.population = population;
        }
    }

    fn remove(&self, id: ServerId) {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
    }
}

pub(crate) async fn listen(
    listener: TcpListener,
    keys: Arc<Keypair>,
    authorized: Arc<HashMap<[u8; 32], ServerId>>,
    servers: Servers,
) {
    loop {
        let Ok((stream, address)) = listener.accept().await else { continue };
        let (keys, authorized, servers) = (keys.clone(), authorized.clone(), servers.clone());
        tokio::spawn(async move {
            if let Err(error) = serve(stream, &keys, &authorized, &servers).await {
                tracing::warn!(%address, %error, "game server channel closed");
            }
        });
    }
}

async fn serve(
    stream: TcpStream,
    keys: &Keypair,
    authorized: &HashMap<[u8; 32], ServerId>,
    servers: &Servers,
) -> Result {
    let mut connection = Connection::accept(stream, keys, Pattern::Peer).await?;
    let Some(&allowed) = connection.remote_key().and_then(|key| authorized.get(&key)) else {
        return Err("a game server with an unauthorized key connected".into());
    };
    let GameToLogin::Register { version, id, name, address, capacity } = connection.recv().await? else {
        return Err("a game server did not register first".into());
    };
    let rejection = if version != VERSION {
        Some(Rejection::UpdateRequired { version: VERSION })
    } else if id != allowed {
        Some(Rejection::WrongId)
    } else if !servers.insert(ServerEntry { id, name: name.clone(), address, population: 0, capacity }) {
        Some(Rejection::AlreadyRegistered)
    } else {
        None
    };
    if let Some(rejection) = rejection {
        connection.send(&LoginToGame::Rejected(rejection)).await?;
        return Err(format!("game server {} was rejected: {rejection:?}", id.0).into());
    }
    tracing::info!(id = id.0, name, "game server registered");
    let result = async {
        connection.send(&LoginToGame::Registered).await?;
        loop {
            match connection.recv().await? {
                GameToLogin::Population(population) => servers.update(id, population),
                GameToLogin::Register { .. } => return Err("a game server registered twice".into()),
            }
        }
    }
    .await;
    servers.remove(id);
    tracing::info!(id = id.0, "game server left");
    result
}

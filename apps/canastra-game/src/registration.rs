//! Staying registered with the login server: register, report population, and reconnect with a growing
//! delay whenever the channel drops.

use std::time::Duration;

use canastra_net::{Connection, Keypair};
use canastra_protocol::registry::{GameToLogin, LoginToGame};
use canastra_protocol::{ServerId, VERSION};
use tokio::net::TcpStream;

use crate::config::{Config, Result};

const FIRST_RETRY: Duration = Duration::from_secs(1);
const LAST_RETRY: Duration = Duration::from_secs(30);
/// How often population is reported while registered.
const REPORT_EVERY: Duration = Duration::from_secs(10);

/// Keeps this server registered until the process stops. Rejections are fatal: retrying cannot fix a
/// wrong id, a duplicate or a version mismatch.
pub(crate) async fn keep_registered(config: &Config, keys: &Keypair, login_key: &[u8; 32]) -> Result {
    let mut retry = FIRST_RETRY;
    loop {
        match session(config, keys, login_key).await {
            Ok(Session::Rejected(rejection)) => {
                return Err(format!("the login server rejected this server: {rejection}").into());
            }
            Ok(Session::Dropped) => retry = FIRST_RETRY,
            Err(error) => tracing::warn!(%error, "cannot reach the login server"),
        }
        tracing::info!(seconds = retry.as_secs(), "reconnecting to the login server");
        tokio::time::sleep(retry).await;
        retry = (retry * 2).min(LAST_RETRY);
    }
}

enum Session {
    Rejected(String),
    /// Registered, then the channel closed.
    Dropped,
}

async fn session(config: &Config, keys: &Keypair, login_key: &[u8; 32]) -> Result<Session> {
    let stream = TcpStream::connect(config.login.address).await?;
    let mut connection = Connection::connect(stream, login_key, Some(keys)).await?;
    connection
        .send(&GameToLogin::Register {
            version: VERSION,
            id: ServerId(config.id),
            name: config.name.clone(),
            address: config.public_address.clone(),
            capacity: config.capacity,
        })
        .await?;
    match connection.recv().await? {
        LoginToGame::Registered => tracing::info!(id = config.id, "registered with the login server"),
        LoginToGame::Rejected(rejection) => return Ok(Session::Rejected(format!("{rejection:?}"))),
    }
    let mut report = tokio::time::interval(REPORT_EVERY);
    loop {
        report.tick().await;
        // ponytail: no players connect yet, so population is always zero.
        if let Err(error) = connection.send(&GameToLogin::Population(0)).await {
            tracing::warn!(%error, "lost the login server");
            return Ok(Session::Dropped);
        }
    }
}

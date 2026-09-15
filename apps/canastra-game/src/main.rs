//! `canastra-game`: a game server. For now it keeps itself registered with the login server and admits
//! players by their tickets; characters and the world come next.

mod admission;
mod config;
mod registration;

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use admission::Admission;
use canastra_protocol::ServerId;
use config::{Config, Result};
use tokio::net::TcpListener;

const USAGE: &str = "usage:
  canastra-game serve [<config.toml>]   run the game server
  canastra-game keygen                  print a [keys] section; authorize its public key on the login server

The configuration defaults to canastra-game.toml.";

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["serve", rest @ ..] if rest.len() <= 1 => {
            serve(Path::new(rest.first().copied().unwrap_or("canastra-game.toml"))).await
        }
        ["keygen"] => config::keygen().map(|keys| print!("{keys}")),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn serve(path: &Path) -> Result {
    let config = Config::load(path)?;
    let (keys, login_key) = (config.keypair()?, config.login_key()?);
    let admission = Arc::new(Admission::new(ServerId(config.id), config.capacity, keys.clone(), config.tickets()?));
    let players = TcpListener::bind(config.players).await?;
    tracing::info!(players = %config.players, "game server listening");
    tokio::spawn(admission::listen(players, admission.clone()));
    tokio::select! {
        result = registration::keep_registered(&config, &keys, &login_key, &admission.online) => result,
        signal = tokio::signal::ctrl_c() => {
            tracing::info!("game server stopping");
            signal.map_err(Into::into)
        }
    }
}

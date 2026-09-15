//! `canastra-game`: a game server. It keeps itself registered with the login server, admits players by
//! their tickets and keeps their characters; the world comes next.

mod config;
mod players;
mod registration;

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use canastra_db::Database;
use canastra_protocol::ServerId;
use config::{Config, Result};
use players::{Admission, Lobby, NameRules, Players};
use tokio::net::TcpListener;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;

const USAGE: &str = "usage:
  canastra-game serve [<config.toml>]   run the game server
  canastra-game keygen                  print a [keys] section; authorize its public key on the login server

The configuration defaults to canastra-game.toml.";

#[tokio::main]
async fn main() -> ExitCode {
    // sqlx reports each migration check as a notice; only its warnings matter here.
    let filter = Targets::new().with_default(Level::INFO).with_target("sqlx", Level::WARN);
    tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).with(filter).init();
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
    let bytes = std::fs::read(&config.game_data).map_err(|error| format!("{}: {error}", config.game_data.display()))?;
    let data =
        canastra_data::format::decode(&bytes).map_err(|error| format!("{}: {error}", config.game_data.display()))?;
    tracing::info!(classes = data.classes.len(), items = data.items.len(), "game data loaded");
    let id = ServerId(config.id);
    let lobby = Lobby {
        server: id,
        database: Database::connect(&config.database_url).await?,
        data,
        names: NameRules::new(&config.characters.name_pattern, &config.characters.forbidden_names)?,
        slots: config.characters.slots,
    };
    let players = Arc::new(Players {
        keys: keys.clone(),
        admission: Admission::new(id, config.capacity, config.tickets()?),
        lobby,
    });
    let listener = TcpListener::bind(config.players).await?;
    tracing::info!(players = %config.players, "game server listening");
    tokio::spawn(players::listen(listener, players.clone()));
    tokio::select! {
        result = registration::keep_registered(&config, &keys, &login_key, &players.admission.online) => result,
        signal = tokio::signal::ctrl_c() => {
            tracing::info!("game server stopping");
            signal.map_err(Into::into)
        }
    }
}

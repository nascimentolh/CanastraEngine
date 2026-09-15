//! `canastra-login`: authenticates players, lists the game servers online and issues the tickets that
//! admit players to them.

mod config;
mod limits;
mod players;
mod registry;
#[cfg(test)]
mod tests;

use std::io::BufRead;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use canastra_db::Database;
use config::{Config, Result};
use limits::Attempts;
use players::Login;
use registry::Servers;
use tokio::net::TcpListener;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;

const USAGE: &str = "usage:
  canastra-login serve [<config.toml>]                 run the login server
  canastra-login create-account <name> [<config.toml>] create an account; the password is read from
                                                       CANASTRA_PASSWORD or the first line of stdin
  canastra-login keygen                                print a [keys] section for a new server

The configuration defaults to canastra-login.toml.";

const DEFAULT_CONFIG: &str = "canastra-login.toml";

#[tokio::main]
async fn main() -> ExitCode {
    // sqlx reports each migration check as a notice; only its warnings matter here.
    let filter = Targets::new().with_default(Level::INFO).with_target("sqlx", Level::WARN);
    tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).with(filter).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["serve", rest @ ..] if rest.len() <= 1 => serve(config_path(rest)).await,
        ["create-account", name, rest @ ..] if rest.len() <= 1 => create_account(name, config_path(rest)).await,
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

fn config_path<'a>(rest: &[&'a str]) -> &'a Path {
    Path::new(rest.first().copied().unwrap_or(DEFAULT_CONFIG))
}

async fn serve(path: &Path) -> Result {
    let config = Config::load(path)?;
    let keys = config.keys()?;
    let database = Database::connect(&config.database_url).await?;
    let players = TcpListener::bind(config.players).await?;
    let game_servers = TcpListener::bind(config.game_servers).await?;
    tracing::info!(players = %config.players, game_servers = %config.game_servers, "login server listening");
    let servers = Servers::default();
    let window = Duration::from_secs(config.limits.window_seconds);
    let login = Arc::new(Login {
        database,
        keys: keys.noise.clone(),
        tickets: keys.tickets,
        ticket_lifetime: Duration::from_secs(config.ticket_seconds),
        servers: servers.clone(),
        by_address: Attempts::new(config.limits.attempts, window),
        by_account: Attempts::new(config.limits.attempts, window),
    });
    tokio::spawn(registry::listen(game_servers, Arc::new(keys.noise), Arc::new(keys.authorized), servers));
    tokio::select! {
        () = players::listen(players, login) => Ok(()),
        signal = tokio::signal::ctrl_c() => {
            tracing::info!("login server stopping");
            signal.map_err(Into::into)
        }
    }
}

async fn create_account(name: &str, path: &Path) -> Result {
    let config = Config::load(path)?;
    let password = match std::env::var("CANASTRA_PASSWORD") {
        Ok(password) => password,
        Err(_) => std::io::stdin().lock().lines().next().transpose()?.unwrap_or_default(),
    };
    if password.is_empty() {
        return Err("the password is empty".into());
    }
    let database = Database::connect(&config.database_url).await?;
    let id = database.create_account(name, &password).await?;
    println!("created account {name} ({})", id.0);
    Ok(())
}

//! `canastra-game`: a game server. For now it only keeps itself registered with the login server, so
//! players see it listed; the world, character selection and tickets come next.

mod config;
mod registration;

use std::path::Path;
use std::process::ExitCode;

use config::{Config, Result};

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
    tokio::select! {
        result = registration::keep_registered(&config, &keys, &login_key) => result,
        signal = tokio::signal::ctrl_c() => {
            tracing::info!("game server stopping");
            signal.map_err(Into::into)
        }
    }
}

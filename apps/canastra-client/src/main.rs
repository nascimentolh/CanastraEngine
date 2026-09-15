//! Canastra client: for now, one UI screen rendered from markup and CSS.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "pixel sizes, scale factors and quad counts stay far inside the exact ranges of the GPU types"
)]

mod app;
mod gpu;
mod lobby;
mod network;
mod renderer;
mod scene;
mod screen;

use std::path::PathBuf;
use std::process::ExitCode;

use winit::event_loop::EventLoop;

const USAGE: &str = "usage: canastra-client <client-root> [<ui-folder>]

Shows login.ui styled by theme.css from <ui-folder> (default assets/ui). F5 reloads both.
CANASTRA_BACKEND=dx12|vulkan|metal forces a graphics backend.
CANASTRA_LOGIN=host:port (default 127.0.0.1:2106) and CANASTRA_LOGIN_KEY=<the login server's noise_public>
point the client at a login server. CANASTRA_GAME_DATA names the game data file (default gamedata.cana).";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (client_root, ui_folder) = match args.as_slice() {
        [client] => (PathBuf::from(client), PathBuf::from("assets/ui")),
        [client, ui] => (PathBuf::from(client), PathBuf::from(ui)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let event_loop = match EventLoop::<network::Reply>::with_user_event().build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let mut app = app::App::new(client_root, ui_folder, event_loop.create_proxy());
    let result = event_loop.run_app(&mut app).map_err(|error| error.to_string());
    match result.err().or(app.error) {
        None => ExitCode::SUCCESS,
        Some(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

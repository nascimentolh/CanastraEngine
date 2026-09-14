//! Canastra client: for now, one UI screen rendered from markup and CSS.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "pixel sizes, scale factors and quad counts stay far inside the exact ranges of the GPU types"
)]

mod app;
mod gpu;
mod renderer;
mod screen;

use std::path::PathBuf;
use std::process::ExitCode;

use winit::event_loop::EventLoop;

const USAGE: &str = "usage: canastra-client <client-root> [<ui-folder>]

Shows login.ui styled by theme.css from <ui-folder> (default assets/ui). F5 reloads both.
CANASTRA_BACKEND=dx12|vulkan|metal forces a graphics backend.";

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
    let mut app = app::App::new(client_root, ui_folder);
    let result = EventLoop::new()
        .map_err(|error| error.to_string())
        .and_then(|event_loop| event_loop.run_app(&mut app).map_err(|error| error.to_string()));
    match result.err().or(app.error) {
        None => ExitCode::SUCCESS,
        Some(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

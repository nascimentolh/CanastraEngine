//! Canastra Studio: edits a `.cana` game data file, showing icons from an H5 client.

mod app;
mod forms;
mod icons;
mod saving;

use std::path::PathBuf;
use std::process::ExitCode;

use eframe::egui;

const USAGE: &str = "usage: canastra-studio <data.cana> [<client-root>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (data_path, client_root) = match args.as_slice() {
        [data] => (PathBuf::from(data), None),
        [data, client] => (PathBuf::from(data), Some(PathBuf::from(client))),
        _ => return fail(USAGE),
    };
    let data = match std::fs::read(&data_path)
        .map_err(|error| error.to_string())
        .and_then(|bytes| canastra_data::format::decode(&bytes).map_err(|error| error.to_string()))
    {
        Ok(data) => data,
        Err(error) => return fail(&format!("{}: {error}", data_path.display())),
    };
    let icons = icons::Icons::new(client_root.as_deref().map(l2_catalog::Catalog::open).unwrap_or_default());

    let mut viewport = egui::ViewportBuilder::default().with_title("Canastra Studio").with_inner_size([1280.0, 800.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../../../icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    let studio = app::Studio::new(data_path, data, icons);
    match eframe::run_native("Canastra Studio", options, Box::new(|_| Ok(Box::new(studio)))) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => fail(&error.to_string()),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::FAILURE
}

// Release builds have no console window on Windows; debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis;
mod app;
mod db;
mod decode;
mod exiftool;
mod filetimes;
mod library;
mod loader;
mod metadata;
mod paths;
mod rating;
mod theme;
mod thumbs;
mod ui;
mod view;

use std::path::PathBuf;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let start_path = std::env::args_os().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Cerno")
            .with_app_id("cerno")
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([480.0, 320.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Cerno",
        options,
        Box::new(move |cc| Ok(Box::new(app::CernoApp::new(cc, start_path)))),
    )
}

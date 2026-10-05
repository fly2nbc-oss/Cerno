// Release builds have no console window on Windows; debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analysis;
mod app;
mod camera_time;
mod crashlog;
mod db;
mod decode;
mod deletion;
mod edit;
mod exiftool;
mod external;
mod filelock;
mod filetimes;
mod histogram;
mod i18n;
mod jpeg_info;
mod library;
mod loader;
mod metadata;
mod name_list;
mod originals;
mod overlay;
mod paths;
mod playback;
mod rating;
mod raw;
mod sidecar;
mod theme;
mod thumbs;
mod transfer;
mod ui;
mod video;
mod view;

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::{egui, wgpu};

fn main() -> eframe::Result {
    let started = Instant::now();
    // Before any thread: the shipped GStreamer's plugin folder and registry (environment).
    playback::configure_environment();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();
    crashlog::install();

    // `cerno --check-video <file>`: the package tests' check that the shipped GStreamer plays.
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--check-video")) {
        let file = std::env::args_os()
            .nth(2)
            .map(PathBuf::from)
            .unwrap_or_default();
        match playback::self_test(&file) {
            Ok(report) => {
                println!("{report}");
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("video check failed: {err}");
                std::process::exit(1);
            }
        }
    }

    let start_path = std::env::args_os().nth(1).map(PathBuf::from);

    // The app – and with it the decoding of the first photo – starts before the window and
    // the GPU are set up; textures uploaded meanwhile are queued by egui until the first frame.
    let ctx = egui::Context::default();
    let app = app::CernoApp::new(&ctx, start_path, started);
    log::info!(
        "start-up: app state ready after {} ms",
        started.elapsed().as_millis()
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Cerno")
            .with_app_id("cerno")
            .with_icon(icon())
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([480.0, 320.0])
            // No `with_maximized`: on a scaled Windows display it leaves the window flagged as
            // maximized at its restored size, and the maximize command of the first frame
            // (`app.rs`) is then a no-op. Measured: the restored window shows for ~13 ms.
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: wgpu_options(),
        ..Default::default()
    };

    eframe::run_native_ext(
        "Cerno",
        options,
        Some(ctx),
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

/// Window and taskbar icon (eframe scales it to the system sizes); without it eframe shows its
/// own "e". The exe's icon in Explorer is `assets/icon.ico`, embedded by `build.rs`. Both come
/// from `tools/make_icon.py`.
fn icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
        .expect("assets/icon.png is a valid PNG")
}

/// One graphics backend instead of probing all of them saves ~90 ms at start-up (OpenGL even
/// opens a helper window). DX12 on Windows – DirectML runs on it anyway; Vulkan with an OpenGL
/// fallback elsewhere. `WGPU_BACKEND` still overrides it.
fn wgpu_options() -> WgpuConfiguration {
    let preferred = if cfg!(windows) {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::VULKAN | wgpu::Backends::GL
    };
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::from_env().unwrap_or(preferred);
    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_is_square_with_transparent_corners() {
        let icon = super::icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        assert_eq!(icon.rgba[3], 0, "top-left corner is transparent");
        let centre = (128 * 256 + 128) * 4;
        assert_eq!(icon.rgba[centre + 3], 255, "the tile is opaque");
    }
}

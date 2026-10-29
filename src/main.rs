//! GetData: a graph digitizer written in Rust with egui.
#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod canvas;
mod coords;
mod dialogs;
mod digitize;
mod dxf;
mod export;
mod gdw;
mod i18n;
mod icons;
mod imageio;
mod model;
mod panels;
mod settings;
mod tiles;
mod ui;
mod view;
mod xlsx;

/// Dock and window icon. `scripts/package-macos.sh` writes this from `assets/icon.png`.
fn app_icon() -> eframe::egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../macos/icon-runtime.png"))
        .expect("bundled app icon must be a valid PNG")
}

fn main() -> eframe::Result {
    let open = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("GetData")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([640.0, 420.0])
            .with_drag_and_drop(true)
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "GetData",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, open)))),
    )
}

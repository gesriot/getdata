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

/// Window icon: a small chart with digitized points.
fn app_icon() -> eframe::egui::IconData {
    let n = 32usize;
    let mut rgba = vec![0u8; n * n * 4];
    let mut put = |x: usize, y: usize, c: [u8; 4]| {
        if x < n && y < n {
            rgba[(y * n + x) * 4..(y * n + x) * 4 + 4].copy_from_slice(&c);
        }
    };
    for y in 2..30 {
        for x in 2..30 {
            put(x, y, [255, 255, 255, 255]);
        }
    }
    for k in 2..30 {
        put(4, k, [40, 40, 40, 255]);
        put(k, 27, [40, 40, 40, 255]);
    }
    for x in 5..29 {
        let t = (x - 5) as f32 / 23.0;
        let y = (25.0 - 20.0 * t.powf(0.6)) as usize;
        put(x, y, [40, 90, 200, 255]);
        put(x, y + 1, [40, 90, 200, 255]);
        if (x - 5) % 5 == 0 {
            for dy in 0..3 {
                for dx in 0..3 {
                    put(x + dx - 1, y + dy - 1, [230, 50, 60, 255]);
                }
            }
        }
    }
    eframe::egui::IconData { rgba, width: n as u32, height: n as u32 }
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

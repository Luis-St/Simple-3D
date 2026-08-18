//! Simple 3D: assemble 3D models out of parametric primitives by typing exact
//! metric dimensions, and export them for slicers.

// A GUI application, not a console one: on Windows this stops a terminal window
// appearing behind it. Debug builds keep the console so panics are visible.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod app_chrome;
mod dock;
// Pointer gestures, executed rather than only reasoned about. Test-only.
#[cfg(test)]
mod gestures;
mod gizmo;
mod icon;
mod panel_outliner;
mod panel_primitives;
mod panel_properties;
mod panel_toolrail;
mod panel_viewport;
mod pick;
mod raster;
mod render;
mod theme;
mod ui;
mod view;
mod worker;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // Opening a project by passing its path as an argument, so file
    // associations work on both platforms (spec section 10).
    let open: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);
    if let Some(path) = &open {
        if !path.exists() {
            eprintln!("{}: no such file", path.display());
        }
    }

    let settings = simple3d_core::config::load_settings();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(app::APP_NAME)
        .with_app_id("net.simple3d.Simple3D")
        .with_inner_size(settings.window_size)
        .with_min_inner_size([900.0, 560.0]);
    if settings.window_maximized {
        viewport = viewport.with_maximized(true);
    }

    let options = eframe::NativeOptions {
        viewport,
        // The viewport is drawn on the CPU, so vsync is all we ask of the
        // graphics stack and there is no shader to fail to compile.
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(app::APP_NAME, options, Box::new(move |cc| Ok(Box::new(app::App::new(&cc.egui_ctx, open)))))
}

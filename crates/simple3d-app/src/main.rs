//! Simple 3D: assemble 3D models from parametric primitives with exact metric dimensions, and
//! export them for slicers.

// No console window on Windows release builds; debug builds keep it so panics are visible.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod app_chrome;
mod components;
mod dock;
// Pointer gestures, executed. Test-only.
#[cfg(test)]
mod gestures;
mod gizmo;
mod gpu;
mod icon;
mod measure_tool;
mod noise_popup;
mod panel_outliner;
mod panel_primitives;
mod panel_properties;
mod panel_toolrail;
mod panel_viewport;
mod pattern_tool;
mod pick;
mod popup;
mod raster;
mod reassemble_tool;
mod render;
mod section_tool;
mod shell;
mod simplify_tool;
mod snap;
mod split_tool;
mod tabs;
mod theme;
mod ui;
mod view;
mod worker;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // A project path argument, so file associations work (spec section 10).
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
        // The window's own icon; otherwise eframe's logo shows on the Windows taskbar.
        .with_icon(icon::app_icon(256))
        .with_inner_size(settings.window_size)
        .with_min_inner_size([900.0, 560.0])
        // Window-system decorations for native snapping, menus and button layout; on GNOME, which lacks
        // `xdg-decoration`, winit draws its Adwaita frame.
        .with_decorations(true);
    if settings.window_maximized {
        viewport = viewport.with_maximized(true);
    }

    let options = eframe::NativeOptions {
        viewport,
        // Swap interval 0 deliberately: with vsync, NVIDIA's Wayland EGL blocks a dialog window's swap
        // forever waiting for a frame callback, freezing the main loop (issue 53). Nothing is lost, since
        // the viewport repaints on demand. (An idle CPU spin once blamed on this was the title being
        // re-sent every frame; see `app/run_loop.rs`.)
        vsync: false,
        ..Default::default()
    };

    eframe::run_native(
        app::APP_NAME,
        options,
        // The shell owns one `App` per window (issue 107).
        Box::new(move |cc| Ok(Box::new(shell::Shell::new(&cc.egui_ctx, cc.gl.clone(), open)))),
    )
}

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
mod gpu;
mod icon;
mod panel_outliner;
mod panel_primitives;
mod panel_properties;
mod panel_toolrail;
mod panel_viewport;
mod pick;
mod raster;
mod render;
mod snap;
mod tabs;
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
        // The window's own icon: what the taskbar, the alt-tab list and the
        // title bar show. eframe hands the window system its own egui logo for
        // any application that does not set one, which is where the letter "e"
        // on the Windows taskbar came from -- the icon compiled into the .exe is
        // what Explorer shows for the file, and never what the running window
        // wears. 256 px is the largest size Windows asks for.
        .with_icon(icon::app_icon(256))
        .with_inner_size(settings.window_size)
        .with_min_inner_size([900.0, 560.0])
        // Decorated by the window system, not by us. Windows, X11, KDE and the
        // wlroots compositors all hand out a real frame -- on Wayland through
        // the `xdg-decoration` protocol -- and with it the snapping, the
        // right-click window menu, the keyboard shortcuts and the button layout
        // the user configured, none of which a drawn title bar can offer.
        // GNOME's mutter still refuses that protocol, so there winit draws the
        // Adwaita frame its `wayland-csd-adwaita` feature provides: an
        // imitation, but a maintained one that follows the desktop's own
        // colour scheme.
        .with_decorations(true);
    if settings.window_maximized {
        viewport = viewport.with_maximized(true);
    }

    let options = eframe::NativeOptions {
        viewport,
        // Swap interval 0, deliberately, and it is not a performance choice.
        //
        // `vsync: true` gives *every* surface swap interval 1, and on NVIDIA's
        // Wayland EGL a swap then waits for that surface's frame callback. A
        // dialog is a real window (issue 53) opened with
        // `show_viewport_immediate`, which eframe renders inside the parent's
        // `update()`; when the compositor scheduled no frames for that second,
        // always-on-top toplevel, its `eglSwapBuffers` waited on a callback
        // that never came -- with `poll(timeout = -1)`, so forever -- and took
        // the main window's event loop down with it. The main window stayed on
        // screen showing its last frame, took no input, and sat at 0% CPU.
        //
        // Nothing is given up by turning it off here. The viewport repaints on
        // demand rather than as fast as it is allowed to, so swap interval 1
        // was never throttling anything; it only gave the swap something to
        // block on.
        //
        // That claim was in doubt for a while: the idle application was
        // measured at 100% of a core two days after this went in, and a
        // repainting loop with nothing left to throttle it was the obvious
        // suspect. It was not the cause. `App::update` was sending the window
        // title on every frame, and `Context::send_viewport_cmd` requests a
        // repaint for every command it is handed, so the frame asked for the
        // next frame and the loop never stopped. Measured with
        // `Context::repaint_causes`, fixed in `app.rs` by sending a title only
        // when it changes, and the idle process now takes 0% of a core with
        // swap interval still 0.
        vsync: false,
        ..Default::default()
    };

    eframe::run_native(
        app::APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(&cc.egui_ctx, cc.gl.clone(), open)))),
    )
}

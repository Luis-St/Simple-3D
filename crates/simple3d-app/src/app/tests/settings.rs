//! What is written to disk, and when.

use super::*;
use simple3d_core::config::{self, Placement};
use simple3d_core::keymap::Command;
use std::time::Duration;

#[test]
pub(crate) fn a_setting_changed_in_the_panel_is_on_disk_before_the_application_closes() {
    // The user set "Add at" to the view centre, the process was killed
    // rather than quit, and the next run opened on the origin again. Only
    // `on_exit` wrote the settings, so anything that ended the process
    // another way -- a crash, a kill, the machine going down -- took every
    // setting changed that session with it. The keymap has been written on
    // the spot since acceptance criterion 28 asked for a rebinding to
    // survive a hard kill; this is the rest of the settings catching up.
    //
    // A real frame is what has to write it, so a real frame is what this
    // draws: calling the writer directly would pass on the broken code,
    // where nothing called it until the application closed.
    let dir = temp_config_dir("settings-survive-a-kill");
    let ctx = egui::Context::default();
    let mut app = app_in(dir.clone());
    assert_eq!(app.settings.placement, Placement::Origin, "the default this test is about has changed");

    // Changed as the panel changes it, and then one frame of the running
    // application -- and then the process is gone.
    app.settings.placement = Placement::ViewCentre;
    app.settings.rotate_snap_deg = 22.5;
    // A window's worth of screen: the default is unbounded, and the
    // viewport would try to rasterize a texture the size of it.
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0))),
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| app.ui(ctx));
    drop(app);

    let next = app_in(dir.clone());
    assert_eq!(next.settings.placement, Placement::ViewCentre, "\"Add at\" was lost with the process");
    assert_eq!(next.settings.rotate_snap_deg, 22.5, "the snap setting was lost with the process");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
pub(crate) fn the_window_size_and_maximized_state_are_remembered_for_the_next_run() {
    // Issue 95. `main` builds the window out of `window_size` and
    // `window_maximized`, and nothing ever wrote either of them back: the
    // window was resized, the application closed cleanly, and the next run
    // opened at the 1400 x 880 default again.
    //
    // Driven through a real frame, because reading the window is something
    // only a frame can do -- calling the writer directly would pass on the
    // broken code, where no frame ever called it.
    let dir = temp_config_dir("window-shape");
    let ctx = egui::Context::default();
    let mut app = app_in(dir.clone());
    assert_eq!(app.settings.window_size, [1400.0, 880.0], "the default this test is about has changed");

    let frame = |app: &mut App, size: egui::Vec2, maximized: bool| {
        let mut viewports = egui::ViewportIdMap::default();
        viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                inner_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                maximized: Some(maximized),
                ..Default::default()
            },
        );
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            viewports,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    };

    frame(&mut app, egui::vec2(1000.0, 700.0), false);
    assert_eq!(app.settings.window_size, [1000.0, 700.0], "the size the window was left at was not recorded");
    assert!(!app.settings.window_maximized);

    // Maximized, the size the window reports is the screen's -- and that is
    // exactly the size it must not come back with once it is restored.
    frame(&mut app, egui::vec2(2560.0, 1440.0), true);
    assert!(app.settings.window_maximized, "the window being maximized was not recorded");
    assert_eq!(app.settings.window_size, [1000.0, 700.0], "the maximized size overwrote the restored size");

    // The write is rate-limited, so the frame that changed something asks
    // for a later one to carry it to disk. That frame is what this is: the
    // running application draws it on its own, and without it the test
    // would be asserting against the gap rather than against the setting.
    std::thread::sleep(Duration::from_millis(300));
    frame(&mut app, egui::vec2(2560.0, 1440.0), true);
    drop(app);

    let next = app_in(dir.clone());
    assert_eq!(next.settings.window_size, [1000.0, 700.0], "the size did not survive the process");
    assert!(next.settings.window_maximized, "the maximized state did not survive the process");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The default `App::new` still points at the user's real config directory --
/// the test seam must not have changed where a shipped binary looks.
#[test]
pub(crate) fn the_default_config_directory_is_the_users_own() {
    let ctx = egui::Context::default();
    let app = App::new(&ctx, None, None);
    assert_eq!(app.config_dir(), config::config_dir().as_path());
}

/// Spec acceptance criterion 19: the application starts and stays usable on a
/// machine with no accelerated graphics.
///
/// There is none here -- no GPU, no window, no display -- and this is the
/// whole of `App::new`: settings, the evaluation worker, the starter scene and
/// the first frame's worth of state. `raster.rs`'s tests cover the drawing
/// that follows being done on the CPU; this covers the starting.
#[test]
pub(crate) fn the_application_starts_with_no_graphics_at_all() {
    let mut app = headless_app();
    assert_eq!(app.status, Status::Idle);
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);

    // And it stays usable: a command runs and takes effect.
    app.run(Command::Duplicate);
    assert_eq!(app.scene.depth_first().len(), 3, "root, plate and its duplicate");
    app.run(Command::Undo);
    assert_eq!(app.scene.depth_first().len(), 2);
}

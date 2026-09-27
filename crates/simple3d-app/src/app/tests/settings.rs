//! What is written to disk, and when.

use super::*;
use simple3d_core::config::{self, Placement};
use simple3d_core::keymap::Command;
use std::time::Duration;

#[test]
pub(crate) fn a_setting_changed_in_the_panel_is_on_disk_before_the_application_closes() {
    // Regression: settings were only written in `on_exit`, so a kill lost them. Driven through a real
    // frame, since calling the writer directly would pass on the broken code.
    let dir = temp_config_dir("settings-survive-a-kill");
    let ctx = egui::Context::default();
    let mut app = app_in(dir.clone());
    assert_eq!(app.settings.placement, Placement::Origin, "the default this test is about has changed");

    // Changed as the panel does, then one frame, then the process is gone.
    app.settings.placement = Placement::ViewCentre;
    app.settings.rotate_snap_deg = 22.5;
    // A bounded screen, since the default is unbounded and the viewport would rasterise all of it.
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
    // Issue 95: window size and maximized state were read but never written back. Driven through
    // real frames, since only a frame reads the window.
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

    // Maximized, the reported size is the screen's, which must not come back once restored.
    frame(&mut app, egui::vec2(2560.0, 1440.0), true);
    assert!(app.settings.window_maximized, "the window being maximized was not recorded");
    assert_eq!(app.settings.window_size, [1000.0, 700.0], "the maximized size overwrote the restored size");

    // Writes are rate-limited, so wait for the follow-up frame that carries it to disk.
    std::thread::sleep(Duration::from_millis(300));
    frame(&mut app, egui::vec2(2560.0, 1440.0), true);
    drop(app);

    let next = app_in(dir.clone());
    assert_eq!(next.settings.window_size, [1000.0, 700.0], "the size did not survive the process");
    assert!(next.settings.window_maximized, "the maximized state did not survive the process");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// `App::new` still uses the real config directory; the test seam changes nothing for shipped builds.
#[test]
pub(crate) fn the_default_config_directory_is_the_users_own() {
    let ctx = egui::Context::default();
    let app = App::new(&ctx, None, None);
    assert_eq!(app.config_dir(), config::config_dir().as_path());
}

/// Spec acceptance criterion 19: the application starts and stays usable without accelerated
/// graphics. This covers `App::new`; `raster.rs`'s tests cover CPU drawing.
#[test]
pub(crate) fn the_application_starts_with_no_graphics_at_all() {
    let mut app = headless_app();
    assert_eq!(app.status, Status::Idle);
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);

    // It stays usable: a command runs and takes effect.
    app.run(Command::Duplicate);
    assert_eq!(app.scene.depth_first().len(), 3, "root, plate and its duplicate");
    app.run(Command::Undo);
    assert_eq!(app.scene.depth_first().len(), 2);
}

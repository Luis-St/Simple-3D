//! Binding keys, and the keymap a restarted application comes back with.

use super::*;
use simple3d_core::keymap::{Chord, Command, Keymap};

/// Issue 76 through the app: the editor records a lone modifier, the binding reads back as it, and
/// holding it requests a snap. The recorder reads its own window's input, which unit tests miss.
#[test]
pub(crate) fn a_modifier_alone_can_be_bound_in_the_editor_and_held_afterwards() {
    let mut app = headless_app();
    // Start from a modifier-free binding, so the recording is new.
    app.keymap.set(Command::ToggleBoundingBox, Chord::key("B"), true).unwrap();
    // Alt alone is the zoom hold's default (issue 97), and a taken chord opens the conflict prompt,
    // so it is freed first.
    app.keymap.unbind(Command::ZoomToPointer);
    app.modal = Modal::Keymap;
    app.recording = Some(Command::ToggleBoundingBox);

    // Alt down, held a frame, then released with nothing under it.
    draw_frame_with(&mut app, egui::Modifiers::ALT, Vec::new());
    assert_eq!(app.recording, Some(Command::ToggleBoundingBox), "the press alone should not finish the binding");
    draw_frame_with(&mut app, egui::Modifiers::ALT, Vec::new());
    draw_frame_with(&mut app, egui::Modifiers::NONE, Vec::new());

    assert_eq!(app.recording, None, "the release should have finished the recording");
    assert_eq!(app.keymap.binding(Command::ToggleBoundingBox), Some(&Chord::modifiers(false, false, true)));
    assert_eq!(app.keymap.shortcut_text(Command::ToggleBoundingBox), "Alt");
    app.modal = Modal::None;

    // Ctrl alone is the default snap hold, and holding it turns snapping on.
    app.keymap = Keymap::default();
    app.settings.geometry_snap = simple3d_core::config::SnapMode::WhileHeld;
    assert_eq!(app.keymap.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));
    assert!(!app.geometry_snap_wanted(|_| false, egui::Modifiers::NONE));
    assert!(app.geometry_snap_wanted(|_| false, egui::Modifiers::COMMAND), "Ctrl on its own did not ask for a snap");
}

/// Issue 76: keys held together are a combination firing on the completing key, not one each.
#[test]
pub(crate) fn keys_held_together_bind_as_one_combination() {
    let mut app = headless_app();
    app.modal = Modal::Keymap;
    app.recording = Some(Command::ToggleBoundingBox);
    let down = |key: egui::Key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    // Q, then W on top, then both released.
    draw_frame_with(&mut app, egui::Modifiers::NONE, vec![down(egui::Key::Q)]);
    draw_frame_with(&mut app, egui::Modifiers::NONE, vec![down(egui::Key::W)]);
    draw_frame_with(&mut app, egui::Modifiers::NONE, Vec::new());
    assert_eq!(app.recording, None, "the release should have finished the recording");
    assert_eq!(app.keymap.shortcut_text(Command::ToggleBoundingBox), "Q+W");
    app.modal = Modal::None;
}

/// Spec acceptance criterion 28: `App::new` picks up a rebinding from a previous run. The file
/// round trip is covered in `config`.
#[test]
pub(crate) fn a_restarted_app_starts_on_the_keymap_the_last_one_saved() {
    let dir = temp_config_dir("restart");

    // First run: rebind and persist, as the editor does.
    let mut first = app_in(dir.clone());
    assert_eq!(first.keymap, Keymap::default(), "a fresh config dir did not give the default keymap");
    first.keymap.set(Command::Group, Chord::ctrl_shift("J"), true).unwrap();
    first.settings.rotate_snap_deg = 7.5;
    first.persist();
    drop(first);

    // Second run: a fresh App over the same directory.
    let second = app_in(dir.clone());
    assert_eq!(second.keymap.binding(Command::Group), Some(&Chord::ctrl_shift("J")), "the rebinding was lost");
    assert_eq!(second.keymap.command_for(&Chord::ctrl_shift("J")), Some(Command::Group));
    assert_eq!(second.settings.rotate_snap_deg, 7.5, "settings did not persist either");
    // The menus show the saved binding.
    assert_ne!(
        second.keymap.shortcut_text(Command::Group),
        Keymap::default().shortcut_text(Command::Group),
        "the menus would still show the default binding"
    );

    // Nothing was written outside the given directory.
    assert_eq!(second.config_dir(), dir.as_path());
    std::fs::remove_dir_all(&dir).unwrap();
}

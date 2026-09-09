//! Binding keys, and the map a restarted application comes back on.

use super::*;
use simple3d_core::keymap::{Chord, Command, Keymap};

/// Issue 76, through the application rather than through `ChordHold` alone:
/// the keymap editor records a modifier held on its own, the binding it
/// writes reads back as that modifier, and holding it afterwards is what
/// asks for a geometry snap.
///
/// The recorder and the shortcut dispatcher watch the same hold from two
/// different places, and the editor's is fed by *its own window's* input --
/// which is exactly the seam a unit test of the hold cannot reach.
#[test]
pub(crate) fn a_modifier_alone_can_be_bound_in_the_editor_and_held_afterwards() {
    let mut app = headless_app();
    // Something with no modifier in it to begin with, so what is recorded
    // cannot be what was already there.
    app.keymap.set(Command::ToggleBoundingBox, Chord::key("B"), true).unwrap();
    // Alt on its own is the zoom hold's default (issue 97), and a recording
    // that lands on a taken chord opens the conflict prompt instead of binding.
    // What is under test here is the recorder, so the chord is freed first.
    app.keymap.unbind(Command::ZoomToPointer);
    app.modal = Modal::Keymap;
    app.recording = Some(Command::ToggleBoundingBox);

    // Alt goes down, is held for a frame, and comes up with nothing under it.
    draw_frame_with(&mut app, egui::Modifiers::ALT, Vec::new());
    assert_eq!(app.recording, Some(Command::ToggleBoundingBox), "the press alone should not finish the binding");
    draw_frame_with(&mut app, egui::Modifiers::ALT, Vec::new());
    draw_frame_with(&mut app, egui::Modifiers::NONE, Vec::new());

    assert_eq!(app.recording, None, "the release should have finished the recording");
    assert_eq!(app.keymap.binding(Command::ToggleBoundingBox), Some(&Chord::modifiers(false, false, true)));
    assert_eq!(app.keymap.shortcut_text(Command::ToggleBoundingBox), "Alt");
    app.modal = Modal::None;

    // And the other half: the default hold for geometry snapping is Ctrl on
    // its own, and holding it is what turns snapping on.
    app.keymap = Keymap::default();
    app.settings.geometry_snap = simple3d_core::config::SnapMode::WhileHeld;
    assert_eq!(app.keymap.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));
    assert!(!app.geometry_snap_wanted(|_| false, egui::Modifiers::NONE));
    assert!(app.geometry_snap_wanted(|_| false, egui::Modifiers::COMMAND), "Ctrl on its own did not ask for a snap");
}

/// The other half of issue 76: an ordinary key held while another goes down
/// is a combination, and it fires on the key that completes it rather than
/// each key firing its own binding on the way.
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
    // Q, then W on top of it, then both let go.
    draw_frame_with(&mut app, egui::Modifiers::NONE, vec![down(egui::Key::Q)]);
    draw_frame_with(&mut app, egui::Modifiers::NONE, vec![down(egui::Key::W)]);
    draw_frame_with(&mut app, egui::Modifiers::NONE, Vec::new());
    assert_eq!(app.recording, None, "the release should have finished the recording");
    assert_eq!(app.keymap.shortcut_text(Command::ToggleBoundingBox), "Q+W");
    app.modal = Modal::None;
}

/// Spec acceptance criterion 28, the last link: `App` startup itself picks up
/// a rebinding written by a previous run.
///
/// `config::a_rebinding_survives_a_restart_and_the_menus_follow` covers the
/// file round trip; this covers `App::new` actually consulting it, which is
/// what makes a restart show the new binding.
#[test]
pub(crate) fn a_restarted_app_starts_on_the_keymap_the_last_one_saved() {
    let dir = temp_config_dir("restart");

    // First run: rebind something and persist, exactly as the keymap editor does.
    let mut first = app_in(dir.clone());
    assert_eq!(first.keymap, Keymap::default(), "a fresh config dir did not give the default keymap");
    first.keymap.set(Command::Group, Chord::ctrl_shift("J"), true).unwrap();
    first.settings.rotate_snap_deg = 7.5;
    first.persist();
    drop(first);

    // Second run: a new App over the same directory, knowing nothing else.
    let second = app_in(dir.clone());
    assert_eq!(second.keymap.binding(Command::Group), Some(&Chord::ctrl_shift("J")), "the rebinding was lost");
    assert_eq!(second.keymap.command_for(&Chord::ctrl_shift("J")), Some(Command::Group));
    assert_eq!(second.settings.rotate_snap_deg, 7.5, "settings did not persist either");
    // What the menus render is the saved binding, not the default.
    assert_ne!(
        second.keymap.shortcut_text(Command::Group),
        Keymap::default().shortcut_text(Command::Group),
        "the menus would still show the default binding"
    );

    // And nothing was written outside the directory we handed it.
    assert_eq!(second.config_dir(), dir.as_path());
    std::fs::remove_dir_all(&dir).unwrap();
}

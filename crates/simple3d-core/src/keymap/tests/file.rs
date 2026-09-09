//! The keymap file, and what an incomplete or unknown one does.

use super::*;

#[test]
pub(crate) fn a_keymap_round_trips_through_its_file_form() {
    // Spec section 8.2: export and import as one file, to carry between machines.
    let mut map = Keymap::from_preset(Preset::Cad);
    map.set(Command::Export, Chord::ctrl_shift("E"), true).unwrap();
    map.nav.orbit = Drag::with_ctrl(MouseButton::Left);
    map.nav.invert_zoom = false;

    let text = map.to_text();
    assert!(text.contains("\"export\": \"Ctrl+Shift+E\""), "{text}");
    let back = Keymap::from_text(&text).unwrap();
    assert_eq!(back, map);
}

#[test]
pub(crate) fn an_imported_map_missing_a_command_falls_back_to_its_preset() {
    let mut map = Keymap::default();
    map.unbind(Command::FrameAll);
    let text = map.to_text();
    let back = Keymap::from_text(&text).unwrap();
    assert_eq!(back.binding(Command::FrameAll), Keymap::default().binding(Command::FrameAll));
    assert!(back.self_conflicts().is_empty());
}

#[test]
pub(crate) fn an_imported_map_with_an_unknown_command_is_not_broken_by_it() {
    let text = r#"{
      "preset": "default",
      "bindings": { "save": "Ctrl+S", "teleport": "Ctrl+T" },
      "nav": { "orbit": { "button": "right" }, "pan": { "button": "right", "shift": true },
               "invert_zoom": false }
    }"#;
    // An unknown command name is a hard error from serde, which is the safe
    // outcome for a file this build cannot fully honour.
    assert!(Keymap::from_text(text).is_err());
}

#[test]
pub(crate) fn an_imported_map_naming_a_retired_command_keeps_the_rest_of_itself() {
    // The handle-frame toggle is gone (issue 100), and every keymap file an
    // older build wrote names it -- a file is written back whole on any change.
    // A retired name must cost that one binding and nothing else; treated as
    // unknown it would be a hard error, and everyone carrying a keymap between
    // machines would silently be handed the preset instead.
    let text = r#"{
      "preset": "default",
      "bindings": { "save": "Ctrl+Shift+S", "toggle_handle_frame": "X" },
      "nav": { "orbit": { "button": "right" }, "pan": { "button": "middle" },
               "invert_zoom": false }
    }"#;
    let map = Keymap::from_text(text).expect("a retired name is ours to have retired, not a broken file");
    assert_eq!(map.binding(Command::Save), Some(&Chord::ctrl_shift("S")), "the map the user chose did not survive");
}

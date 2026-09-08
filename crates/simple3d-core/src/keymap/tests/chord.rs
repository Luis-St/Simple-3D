//! Chords: how they read, and which one a set of held keys fires.

use super::*;
use std::str::FromStr;

#[test]
pub(crate) fn chords_round_trip_through_their_display_form() {
    for chord in [
        Chord::key("A"),
        Chord::ctrl("S"),
        Chord::ctrl_shift("Z"),
        Chord::shift("Up"),
        Chord { keys: vec!["F5".into()], ctrl: true, shift: true, alt: true },
        Chord::combo(["Q", "W", "E"]),
    ] {
        assert_eq!(Chord::from_str(&chord.to_string()).unwrap(), chord);
    }
    assert_eq!(Chord::from_str("Cmd+S").unwrap(), Chord::ctrl("S"));
    assert!(Chord::from_str("").is_err());
    assert!(Chord::from_str("Ctrl+").is_err());
}

#[test]
pub(crate) fn drag_modifiers_must_match_exactly() {
    let pan = Drag::with_shift(MouseButton::Right);
    assert!(pan.matches(MouseButton::Right, false, true, false));
    assert!(!pan.matches(MouseButton::Right, false, false, false));
    assert!(!pan.matches(MouseButton::Right, true, true, false));
    let orbit = Drag::new(MouseButton::Right);
    assert!(!orbit.matches(MouseButton::Right, false, true, false), "orbit fired on the pan chord");
}

#[test]
pub(crate) fn shortcut_text_is_what_the_menus_show() {
    let map = Keymap::default();
    assert_eq!(map.shortcut_text(Command::Save), "Ctrl+S");
    assert_eq!(map.shortcut_text(Command::SaveAs), "Ctrl+Shift+S");
    assert_eq!(map.shortcut_text(Command::Delete), "Delete");
}

#[test]
pub(crate) fn a_modifier_on_its_own_is_a_chord() {
    // Issue 77: Ctrl, Shift and Alt used to be unbindable because a chord
    // was required to carry a key as well.
    let ctrl = Chord::modifiers(true, false, false);
    assert!(ctrl.is_modifier_only());
    assert_eq!(ctrl.to_string(), "Ctrl");
    assert_eq!(Chord::from_str("Ctrl").unwrap(), ctrl);

    let all = Chord::modifiers(true, true, true);
    assert_eq!(all.to_string(), "Ctrl+Alt+Shift");
    assert_eq!(Chord::from_str(&all.to_string()).unwrap(), all);

    // A key chord is still not modifier-only, and nothing at all is still an
    // error.
    assert!(!Chord::ctrl("S").is_modifier_only());
    assert!(Chord::from_str("").is_err());
    assert!(Chord::from_str("Ctrl+").is_err());
}

#[test]
pub(crate) fn a_modifier_only_chord_is_a_binding_of_its_own_not_a_prefix() {
    // Ctrl and Ctrl+S are different bindings: holding Ctrl to snap must not
    // collide with Save, in either direction.
    let map = Keymap::default();
    assert_eq!(map.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));
    assert_eq!(map.command_for(&Chord::ctrl("S")), Some(Command::Save));
    assert_eq!(map.command_for(&Chord::modifiers(true, false, false)), Some(Command::SnapToGeometry));
    assert!(map.self_conflicts().is_empty());
    assert_eq!(map.shortcut_text(Command::SnapToGeometry), "Ctrl");
}

#[test]
pub(crate) fn a_keymap_holding_a_modifier_only_binding_round_trips() {
    let mut map = Keymap::default();
    map.set(Command::ToggleBoundingBox, Chord::modifiers(false, true, true), true).unwrap();
    let text = map.to_text();
    assert!(text.contains("\"snap_to_geometry\": \"Ctrl\""), "{text}");
    assert!(text.contains("\"toggle_bounding_box\": \"Alt+Shift\""), "{text}");
    assert_eq!(Keymap::from_text(&text).unwrap(), map);
}

#[test]
pub(crate) fn a_combination_of_ordinary_keys_is_a_chord_too() {
    // Any key can be the base of a combination, not only Ctrl, Shift and
    // Alt: Q+W+E is a binding.
    let mut map = Keymap::default();
    let combo = Chord::combo(["Q", "W", "E"]);
    assert_eq!(combo.to_string(), "E+Q+W", "the keys are a set, written in one settled order");
    assert_eq!(Chord::from_str("Q+W+E").unwrap(), combo, "the order they are written in must not matter");
    map.set(Command::FrameAll, combo.clone(), true).unwrap();

    let down = |name: &str| ["Q", "W", "E"].contains(&name);
    assert_eq!(map.command_for_press("E", down, false, false, false), Some(Command::FrameAll));
    // The press that completes it is the one that fires. Q and W are in the
    // chord too, but pressing them again with everything already down is not
    // a second completion of a different binding.
    assert_eq!(map.command_for_press("Q", down, false, false, false), Some(Command::FrameAll));
    // A key of the combination pressed on its own does not fire it -- it
    // fires whatever that key is bound to by itself, which here is the
    // preset's own rotate.
    assert_eq!(map.command_for_press("E", |name| name == "E", false, false, false), Some(Command::ModeRotate));
    // Nor does the combination fire with a modifier held that it does not
    // carry: Ctrl+E is Export, and stays Export with Q and W down.
    assert_eq!(map.command_for_press("E", down, true, false, false), Some(Command::Export));
}

#[test]
pub(crate) fn the_longest_binding_the_held_keys_satisfy_is_the_one_that_fires() {
    // With Q and W down, pressing E fires Q+W+E rather than whatever E alone
    // is bound to -- otherwise a combination could never be built out of
    // keys that are already in use.
    let mut map = Keymap::default();
    map.set(Command::FrameSelection, Chord::key("E"), true).unwrap();
    map.set(Command::FrameAll, Chord::combo(["Q", "W", "E"]), true).unwrap();

    let all_down = |name: &str| ["Q", "W", "E"].contains(&name);
    assert_eq!(map.command_for_press("E", all_down, false, false, false), Some(Command::FrameAll));
    assert_eq!(map.command_for_press("E", |name| name == "E", false, false, false), Some(Command::FrameSelection));
}

#[test]
pub(crate) fn a_chord_resolves_back_to_its_command() {
    let map = Keymap::default();
    assert_eq!(map.command_for(&Chord::ctrl("S")), Some(Command::Save));
    assert_eq!(map.command_for(&Chord::ctrl_shift("S")), Some(Command::SaveAs));
    assert_eq!(map.command_for(&Chord::key("Backslash")), None);
}

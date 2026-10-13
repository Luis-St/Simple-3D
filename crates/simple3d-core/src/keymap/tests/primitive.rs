//! Shortcuts that add a primitive (issue 124).

use super::*;

#[test]
pub(crate) fn a_primitive_shortcut_fires_and_conflicts_with_commands_both_ways() {
    let mut map = Keymap::default();
    let free = Chord::alt("F8");
    assert_eq!(map.holder(&free, None), None, "pick a chord no preset binds");
    map.set_primitive("cylinder", free.clone(), false).unwrap();
    assert_eq!(
        map.action_for_press("F8", |k| k == "F8", false, false, true),
        Some(Action::Primitive("cylinder".into()))
    );
    assert_eq!(map.primitive_shortcut_text("cylinder"), free.to_string());

    // Onto a command's chord: the command is named, and nothing changes until forced.
    let save = map.binding(Command::Save).unwrap().clone();
    assert_eq!(map.set_primitive("box", save.clone(), false), Err(Action::Command(Command::Save)));
    assert_eq!(map.primitive_binding("box"), None);
    map.set_primitive("box", save.clone(), true).unwrap();
    assert_eq!(map.binding(Command::Save), None);

    // And a command onto a primitive's chord names the primitive.
    let holder = map.set(Command::Export, free.clone(), false).unwrap_err();
    assert_eq!(holder, Action::Primitive("cylinder".into()));
    assert_eq!(holder.label(), "Add Cylinder");
    map.set(Command::Export, free, true).unwrap();
    assert_eq!(map.primitive_binding("cylinder"), None);
}

#[test]
pub(crate) fn primitive_shortcuts_are_saved_and_survive_a_preset_switch_unless_it_takes_their_chord() {
    let mut map = Keymap::default();
    assert!(!map.to_text().contains("primitives"), "an empty map is left out of the file");
    map.set_primitive("sphere", Chord::alt("F8"), false).unwrap();
    map.set_primitive("no_such_shape", Chord::alt("F9"), false).unwrap();
    let read = Keymap::from_text(&map.to_text()).unwrap();
    assert_eq!(read.primitive_binding("sphere"), Some(&Chord::alt("F8")));
    assert_eq!(read.primitive_binding("no_such_shape"), None, "an unknown shape's shortcut was kept");

    let mut switched = read.clone();
    let taken = Keymap::from_preset(Preset::MeshEditor).binding(Command::ModeMove).unwrap().clone();
    switched.primitives.insert("box".into(), taken);
    switched.switch_preset(Preset::MeshEditor);
    assert_eq!(switched.primitive_binding("sphere"), Some(&Chord::alt("F8")));
    assert_eq!(switched.primitive_binding("box"), None, "a primitive kept a chord the preset binds");
    assert!(switched.holder(&Chord::key("G"), Some(&Action::Command(Command::ModeMove))).is_none());
}

//! Rebinding, conflicts and resetting.

use super::*;

#[test]
pub(crate) fn rebinding_onto_a_used_combination_names_the_holder() {
    // Spec section 8.2: a conflict names the holder and offers reassign or cancel, never overwriting silently.
    let mut map = Keymap::default();
    let save = map.binding(Command::Save).unwrap().clone();
    let holder = map.set(Command::Export, save.clone(), false).unwrap_err();
    assert_eq!(holder, Command::Save);
    // Nothing changes while the user decides.
    assert_eq!(map.binding(Command::Save), Some(&save));
    assert_ne!(map.binding(Command::Export), Some(&save));

    // Reassigning takes it from the previous holder.
    map.set(Command::Export, save.clone(), true).unwrap();
    assert_eq!(map.binding(Command::Export), Some(&save));
    assert_eq!(map.binding(Command::Save), None);
    assert!(map.self_conflicts().is_empty());
}

/// Spec acceptance criterion 27 in sequence: switch preset, then rebind onto a used chord, and the
/// conflict is named against the new preset's bindings.
#[test]
pub(crate) fn a_conflict_is_named_after_switching_preset_too() {
    let mut map = Keymap::default();
    map.switch_preset(Preset::MeshEditor);
    assert_eq!(map.preset, Preset::MeshEditor);
    assert!(map.self_conflicts().is_empty(), "the switch itself introduced a conflict");

    // A chord this preset really holds, not one carried over.
    let (holder, chord) = (Command::ModeMove, map.binding(Command::ModeMove).unwrap().clone());
    assert_eq!(chord, Chord::key("G"), "MeshEditor's ModeMove binding changed; pick another chord");

    let named = map.set(Command::Rename, chord.clone(), false).unwrap_err();
    assert_eq!(named, holder, "the conflict named the wrong command");
    // Refused, not overwritten: both bindings unchanged.
    assert_eq!(map.binding(Command::ModeMove), Some(&chord));
    assert_ne!(map.binding(Command::Rename), Some(&chord));

    // Reassigning on confirmation takes it from the holder.
    map.set(Command::Rename, chord.clone(), true).unwrap();
    assert_eq!(map.binding(Command::Rename), Some(&chord));
    assert_eq!(map.binding(Command::ModeMove), None);
    assert!(map.self_conflicts().is_empty());
}

#[test]
pub(crate) fn rebinding_a_command_to_its_own_chord_is_not_a_conflict() {
    let mut map = Keymap::default();
    let save = map.binding(Command::Save).unwrap().clone();
    assert!(map.set(Command::Save, save, false).is_ok());
}

#[test]
pub(crate) fn a_single_binding_resets_to_the_preset_default() {
    let mut map = Keymap::default();
    let original = map.binding(Command::Save).unwrap().clone();
    map.set(Command::Save, Chord::ctrl_shift("F9"), true).unwrap();
    assert_ne!(map.binding(Command::Save), Some(&original));
    map.reset(Command::Save);
    assert_eq!(map.binding(Command::Save), Some(&original));
    assert!(map.self_conflicts().is_empty());
}

#[test]
pub(crate) fn resetting_a_binding_takes_it_back_from_whoever_holds_it() {
    let mut map = Keymap::default();
    let save = map.binding(Command::Save).unwrap().clone();
    map.set(Command::Export, save.clone(), true).unwrap();
    map.reset(Command::Save);
    assert_eq!(map.binding(Command::Save), Some(&save));
    assert!(map.self_conflicts().is_empty());
}

#[test]
pub(crate) fn the_whole_map_resets_to_the_preset() {
    let mut map = Keymap::from_preset(Preset::MeshEditor);
    let pristine = map.clone();
    map.set(Command::Save, Chord::key("F9"), true).unwrap();
    map.set(Command::ModeMove, Chord::key("F10"), true).unwrap();
    map.reset_all();
    assert_eq!(map, pristine);
}

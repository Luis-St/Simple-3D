//! Recording a chord from held keys.

use super::*;
use simple3d_core::keymap::{Chord, Command, Keymap};

#[test]
pub(crate) fn menu_labels_show_the_current_binding() {
    let mut keymap = Keymap::default();
    assert_eq!(menu_label(&keymap, Command::Save), "Save\tCtrl+S");
    keymap.set(Command::Save, Chord::key("F9"), true).unwrap();
    assert_eq!(menu_label(&keymap, Command::Save), "Save\tF9");
    keymap.unbind(Command::Save);
    assert_eq!(menu_label(&keymap, Command::Save), "Save");
}

#[test]
pub(crate) fn an_egui_key_press_resolves_to_its_command() {
    // The names the toolkit gives its keys are the names the keymap stores,
    // which is what lets a press be looked up at all.
    let keymap = Keymap::default();
    let press = |key: egui::Key, modifiers: egui::Modifiers| {
        let name = key.name();
        keymap.command_for_press(name, |k| k == name, modifiers.command, modifiers.shift, modifiers.alt)
    };
    assert_eq!(press(egui::Key::S, egui::Modifiers::COMMAND), Some(Command::Save));
    assert_eq!(press(egui::Key::S, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT), Some(Command::SaveAs));
    assert_eq!(press(egui::Key::Delete, egui::Modifiers::NONE), Some(Command::Delete));
    assert_eq!(press(egui::Key::ArrowUp, egui::Modifiers::NONE), Some(Command::NudgeUp));
}

#[test]
pub(crate) fn a_modifier_released_on_its_own_records_as_a_chord() {
    // Issue 77: the toolkit reports no key event for Ctrl, so a modifier
    // press is recognised from the state going down and coming back up with
    // nothing under it.
    let ctrl = egui::Modifiers::COMMAND;
    let none = egui::Modifiers::NONE;
    let mut hold = ChordHold::default();
    assert_eq!(hold.update(ctrl, UP, false), None, "the press alone is not yet a chord");
    assert_eq!(hold.update(ctrl, UP, false), None, "still held");
    assert_eq!(hold.update(none, UP, false), Some(Chord::modifiers(true, false, false)));
    // The hold is spent: an idle frame afterwards fires nothing.
    assert_eq!(hold.update(none, UP, false), None);
}

#[test]
pub(crate) fn a_modifier_held_under_another_key_records_as_the_combination() {
    // Ctrl+S has to come out as Ctrl+S, not as Ctrl and then S.
    let ctrl = egui::Modifiers::COMMAND;
    let none = egui::Modifiers::NONE;
    let mut hold = ChordHold::default();
    hold.update(ctrl, UP, false);
    hold.update(ctrl, ["S"], false);
    assert_eq!(hold.update(none, UP, false), Some(Chord::ctrl("S")));
}

#[test]
pub(crate) fn several_ordinary_keys_held_together_are_one_chord() {
    // Q, then W, then E, all let go: one binding, whatever order they came
    // off in. The keys are a set, so the same three in another order are the
    // same chord and cannot be bound twice.
    let none = egui::Modifiers::NONE;
    let mut hold = ChordHold::default();
    hold.update(none, ["Q"], false);
    hold.update(none, ["Q", "W"], false);
    hold.update(none, ["Q", "W", "E"], false);
    hold.update(none, ["W", "E"], false);
    let chord = hold.update(none, UP, false).expect("the release completes the chord");
    assert_eq!(chord, Chord::combo(["E", "Q", "W"]));
    assert_eq!(chord.to_string(), "E+Q+W");
    assert_eq!(chord, Chord::combo(["W", "E", "Q"]), "the order keys were pressed in made a different chord");
}

#[test]
pub(crate) fn a_hold_over_a_mouse_gesture_is_the_gesture_not_a_chord() {
    // Ctrl+click picks a second object, and must not also fire whatever Ctrl
    // alone is bound to.
    let mut hold = ChordHold::default();
    hold.update(egui::Modifiers::COMMAND, UP, false);
    hold.update(egui::Modifiers::COMMAND, UP, true);
    assert_eq!(hold.update(egui::Modifiers::NONE, UP, false), None, "a Ctrl+click fired the Ctrl binding");
}

#[test]
pub(crate) fn a_hold_the_keyboard_left_mid_way_is_forgotten() {
    let mut hold = ChordHold::default();
    hold.update(egui::Modifiers::ALT, UP, false);
    hold.reset();
    assert_eq!(hold.update(egui::Modifiers::NONE, UP, false), None, "a dropped hold fired on the release");
}

#[test]
pub(crate) fn every_default_binding_can_be_produced_by_a_real_key_press() {
    // A binding nobody can type would be a silent dead end, so check every
    // preset's key names against the toolkit's own key list.
    let names: Vec<&'static str> = egui::Key::ALL.iter().map(|k| k.name()).collect();
    for preset in simple3d_core::keymap::Preset::ALL {
        let keymap = Keymap::from_preset(preset);
        for command in Command::ALL {
            let chord = keymap.binding(*command).unwrap();
            // A chord that is modifiers alone names no key, and is typed by
            // holding and releasing the modifier (issue 77).
            if chord.is_modifier_only() {
                continue;
            }
            assert!(
                chord.keys.iter().all(|k| names.contains(&k.as_str())),
                "{preset:?}: {:?} is bound to {:?}, which is not a key that exists",
                command,
                chord.keys
            );
        }
    }
}

use super::chord::*;
use super::commit::*;
use super::describe::*;
use super::menu::*;
use simple3d_core::keymap::{Chord, Command, Keymap};
use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::unit::Unit;

#[test]
fn a_length_is_read_in_the_display_unit() {
    assert_eq!(
        commit_param("1.8", ParamKind::Length { min: 0.0 }, Unit::Metre, 0.0),
        Commit::Value(ParamValue::Length(1800.0))
    );
    assert_eq!(
        commit_param("1,8", ParamKind::Length { min: 0.0 }, Unit::Millimetre, 0.0),
        Commit::Value(ParamValue::Length(1.8))
    );
}

#[test]
fn garbage_reverts_rather_than_raising_a_dialog() {
    // Spec acceptance criterion 14.
    for bad in ["", "  ", "abc", "1.2.3", "--4", "NaN", "inf", "12mmm"] {
        assert_eq!(
            commit_param(bad, ParamKind::Length { min: 0.0 }, Unit::Millimetre, 10.0),
            Commit::Revert,
            "{bad:?}"
        );
        assert_eq!(commit_length(bad, Unit::Millimetre, 10.0), None, "{bad:?}");
        assert_eq!(commit_angle(bad, 10.0), None, "{bad:?}");
    }
}

#[test]
fn a_rotation_comes_back_as_the_direction_it_faces_not_the_turn_that_got_there() {
    // Issue 84: a field on a rotation reads [0, 360). Typing a whole turn
    // past where the body already stands leaves it standing there.
    assert_eq!(commit_angle("400", 0.0), Some(40.0));
    assert_eq!(commit_angle("360", 0.0), Some(0.0));
    assert_eq!(commit_angle("-90", 0.0), Some(270.0));
    // The relative forms are resolved first: a degree on from 359 is 0,
    // which is the case the issue names.
    assert_eq!(commit_angle("+1", 359.0), Some(0.0));
    assert_eq!(commit_angle("- 1", 0.0), Some(359.0));
}

#[test]
fn a_dimension_below_its_minimum_is_clamped_not_rejected() {
    let kind = ParamKind::Length { min: 1e-3 };
    assert_eq!(commit_param("0", kind, Unit::Millimetre, 4.0), Commit::Value(ParamValue::Length(1e-3)));
    assert_eq!(commit_param("-5", kind, Unit::Millimetre, 4.0), Commit::Value(ParamValue::Length(1e-3)));
    assert_eq!(commit_param("5", kind, Unit::Millimetre, 4.0), Commit::Value(ParamValue::Length(5.0)));
}

#[test]
fn a_count_is_rounded_and_clamped_to_its_range() {
    let kind = ParamKind::Count { min: 3, max: 128 };
    assert_eq!(commit_param("6", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(6)));
    assert_eq!(commit_param("6.7", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(7)));
    assert_eq!(commit_param("1", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(3)));
    assert_eq!(commit_param("999", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(128)));
    assert_eq!(commit_param("-4", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(3)));
}

#[test]
fn an_angle_is_clamped_to_its_range_and_stays_in_degrees() {
    let kind = ParamKind::Angle { min: 1.0, max: 360.0 };
    // Angles are always degrees regardless of the length unit.
    assert_eq!(commit_param("90", kind, Unit::Metre, 45.0), Commit::Value(ParamValue::Angle(90.0)));
    assert_eq!(commit_param("999", kind, Unit::Millimetre, 45.0), Commit::Value(ParamValue::Angle(360.0)));
    assert_eq!(commit_param("0", kind, Unit::Millimetre, 45.0), Commit::Value(ParamValue::Angle(1.0)));
}

#[test]
fn a_choice_index_cannot_run_off_the_end() {
    let kind = ParamKind::Choice { options: &["Across corners", "Across flats"] };
    assert_eq!(commit_param("1", kind, Unit::Millimetre, 0.0), Commit::Value(ParamValue::Choice(1)));
    assert_eq!(commit_param("7", kind, Unit::Millimetre, 0.0), Commit::Value(ParamValue::Choice(1)));
    assert_eq!(commit_param("-1", kind, Unit::Millimetre, 0.0), Commit::Value(ParamValue::Choice(0)));
}

#[test]
fn what_a_field_shows_round_trips_back_through_what_it_accepts() {
    for (value, unit) in [
        (ParamValue::Length(40.0), Unit::Millimetre),
        (ParamValue::Length(4.0), Unit::Metre),
        (ParamValue::Length(0.5), Unit::Centimetre),
        (ParamValue::Angle(180.0), Unit::Millimetre),
        (ParamValue::Count(6), Unit::Millimetre),
    ] {
        let shown = show_param(value, unit);
        let kind = match value {
            ParamValue::Length(_) => ParamKind::Length { min: 0.0 },
            ParamValue::Angle(_) => ParamKind::Angle { min: -360.0, max: 360.0 },
            ParamValue::Count(_) => ParamKind::Count { min: 0, max: 1000 },
            ParamValue::Choice(_) => ParamKind::Choice { options: &["a", "b"] },
            ParamValue::Bool(_) => ParamKind::Bool,
        };
        assert_eq!(
            commit_param(&shown, kind, unit, param_number(value)),
            Commit::Value(value),
            "{shown:?} in {unit:?}"
        );
    }
}

#[test]
fn shown_values_never_carry_floating_point_noise() {
    assert_eq!(show_param(ParamValue::Length(4.0), Unit::Metre), "0.004");
    assert_eq!(show_param(ParamValue::Length(0.1 + 0.2), Unit::Millimetre), "0.3");
    assert_eq!(show_param(ParamValue::Length(1800.0), Unit::Metre), "1.8");
}

#[test]
fn menu_labels_show_the_current_binding() {
    let mut keymap = Keymap::default();
    assert_eq!(menu_label(&keymap, Command::Save), "Save\tCtrl+S");
    keymap.set(Command::Save, Chord::key("F9"), true).unwrap();
    assert_eq!(menu_label(&keymap, Command::Save), "Save\tF9");
    keymap.unbind(Command::Save);
    assert_eq!(menu_label(&keymap, Command::Save), "Save");
}

#[test]
fn an_egui_key_press_resolves_to_its_command() {
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

/// A frame with nothing held: the state a hold is completed by.
const UP: [&str; 0] = [];

#[test]
fn a_modifier_released_on_its_own_records_as_a_chord() {
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
fn a_modifier_held_under_another_key_records_as_the_combination() {
    // Ctrl+S has to come out as Ctrl+S, not as Ctrl and then S.
    let ctrl = egui::Modifiers::COMMAND;
    let none = egui::Modifiers::NONE;
    let mut hold = ChordHold::default();
    hold.update(ctrl, UP, false);
    hold.update(ctrl, ["S"], false);
    assert_eq!(hold.update(none, UP, false), Some(Chord::ctrl("S")));
}

#[test]
fn several_ordinary_keys_held_together_are_one_chord() {
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
fn a_hold_over_a_mouse_gesture_is_the_gesture_not_a_chord() {
    // Ctrl+click picks a second object, and must not also fire whatever Ctrl
    // alone is bound to.
    let mut hold = ChordHold::default();
    hold.update(egui::Modifiers::COMMAND, UP, false);
    hold.update(egui::Modifiers::COMMAND, UP, true);
    assert_eq!(hold.update(egui::Modifiers::NONE, UP, false), None, "a Ctrl+click fired the Ctrl binding");
}

#[test]
fn a_hold_the_keyboard_left_mid_way_is_forgotten() {
    let mut hold = ChordHold::default();
    hold.update(egui::Modifiers::ALT, UP, false);
    hold.reset();
    assert_eq!(hold.update(egui::Modifiers::NONE, UP, false), None, "a dropped hold fired on the release");
}

#[test]
fn every_default_binding_can_be_produced_by_a_real_key_press() {
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

#[test]
fn a_bounding_box_reads_as_three_dimensions_in_the_display_unit() {
    let size = simple3d_geom::Vec3::new(40.0, 20.0, 4.0);
    assert_eq!(describe_size(size, Unit::Millimetre), "40 x 20 x 4 mm");
    assert_eq!(describe_size(size, Unit::Metre), "0.04 x 0.02 x 0.004 m");
}

#[test]
fn counts_and_durations_read_naturally() {
    assert_eq!(describe_counts(1, 1), "1 node  1 triangle");
    assert_eq!(describe_counts(3, 240), "3 nodes  240 triangles");
    assert_eq!(describe_elapsed(std::time::Duration::from_millis(12)), "12 ms");
    // Issue 38: the readout used to say "0 ms" for every evaluation that
    // took less than half a millisecond, which is most of them.
    assert_eq!(describe_elapsed(std::time::Duration::from_micros(240)), "0.24 ms");
    assert_eq!(describe_elapsed(std::time::Duration::from_millis(2500)), "2.5 s");
}

//! Shared interface pieces, with the decision-making parts kept as pure
//! functions so they can be tested without an egui context.

use scadstudio_core::keymap::{Chord, Command, Keymap};
use scadstudio_core::primitive::{ParamKind, ParamValue};
use scadstudio_core::unit::{format_angle, format_length, format_number, parse_number, Unit};
use std::collections::HashMap;

/// What a text field's content should do to the model when the user commits it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Commit {
    /// A new value to store.
    Value(ParamValue),
    /// Unparseable: restore the previous value silently, no dialog (spec
    /// section 4, acceptance criterion 14).
    Revert,
}

/// Interpret typed text for a parameter of the given kind. Out-of-range values
/// are clamped rather than rejected -- the user's intent is clear, and refusing a
/// number they can see in the field is more confusing than adjusting it.
pub fn commit_param(text: &str, kind: ParamKind, unit: Unit) -> Commit {
    let Some(raw) = parse_number(text) else { return Commit::Revert };
    match kind {
        ParamKind::Length { min } => {
            let mm = unit.to_mm(raw);
            if mm < min {
                Commit::Value(ParamValue::Length(min))
            } else {
                Commit::Value(ParamValue::Length(mm))
            }
        }
        ParamKind::Angle { min, max } => Commit::Value(ParamValue::Angle(raw.clamp(min, max))),
        ParamKind::Count { min, max } => {
            if raw < 0.0 {
                return Commit::Value(ParamValue::Count(min));
            }
            Commit::Value(ParamValue::Count((raw.round() as u32).clamp(min, max)))
        }
        ParamKind::Bool => Commit::Value(ParamValue::Bool(raw != 0.0)),
        ParamKind::Choice { options } => {
            let index = (raw.max(0.0) as u32).min(options.len().saturating_sub(1) as u32);
            Commit::Value(ParamValue::Choice(index))
        }
    }
}

/// A position or rotation component. Positions may be negative, so there is no
/// minimum; rotations are free too, since a node may be turned any way.
pub fn commit_length(text: &str, unit: Unit) -> Option<f64> {
    parse_number(text).map(|v| unit.to_mm(v))
}

pub fn commit_angle(text: &str) -> Option<f64> {
    parse_number(text)
}

/// How a parameter's current value is shown in its field.
pub fn show_param(value: ParamValue, unit: Unit) -> String {
    match value {
        ParamValue::Length(mm) => format_length(mm, unit),
        ParamValue::Angle(deg) => format_angle(deg),
        ParamValue::Count(n) => n.to_string(),
        ParamValue::Choice(n) => n.to_string(),
        ParamValue::Bool(b) => b.to_string(),
    }
}

/// Text fields keep their own in-progress buffer while focused, so a half-typed
/// `1.` is not parsed as it is typed and does not fight the value the model
/// holds. Keyed by widget id.
#[derive(Default)]
pub struct FieldBuffers(HashMap<egui::Id, String>);

impl FieldBuffers {
    /// Draw a single-line field. Returns the committed text when the user
    /// presses Enter or leaves the field, and `None` while they are still typing.
    ///
    /// The value shown comes from the model whenever the field is not being
    /// edited, which is what makes a manipulator drag update the property editor
    /// live and typing move the handles -- one source of truth, both directions.
    pub fn field(&mut self, ui: &mut egui::Ui, id: egui::Id, current: &str) -> Option<String> {
        let mut text = self.0.get(&id).cloned().unwrap_or_else(|| current.to_string());
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(id)
                .desired_width(f32::INFINITY)
                .horizontal_align(egui::Align::RIGHT),
        );
        let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if response.has_focus() && !entered {
            self.0.insert(id, text);
            return None;
        }
        if response.lost_focus() || entered {
            let committed = self.0.remove(&id).unwrap_or(text);
            if committed != current {
                return Some(committed);
            }
            return None;
        }
        if response.changed() {
            self.0.insert(id, text);
        }
        None
    }

    /// Forget every in-progress edit, for when the selection changes underneath.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// The label to put on a menu entry: the command's name plus its *current*
/// binding, never a hardcoded one (spec section 8.2).
pub fn menu_label(keymap: &Keymap, command: Command) -> String {
    match keymap.binding(command) {
        Some(chord) => format!("{}\t{}", command.label(), chord),
        None => command.label().to_string(),
    }
}

/// Translate an egui key press into the chord form the keymap stores. Uses
/// egui's own key names, so there is no translation table to drift out of date.
pub fn chord_from_egui(key: egui::Key, modifiers: egui::Modifiers) -> Chord {
    Chord {
        key: key.name().to_string(),
        // `Modifiers::command` is Ctrl on Windows and Linux and Cmd on macOS, so
        // a keymap saved on one platform loads sensibly on the other.
        ctrl: modifiers.command,
        shift: modifiers.shift,
        alt: modifiers.alt,
    }
}

/// A short description of a bounding box, for the overlay that answers "will
/// this fit" (spec section 6.1).
pub fn describe_size(size: scadstudio_geom::Vec3, unit: Unit) -> String {
    format!(
        "{} x {} x {} {}",
        format_length(size.x, unit),
        format_length(size.y, unit),
        format_length(size.z, unit),
        unit.suffix()
    )
}

/// The status bar's triangle and node counts.
pub fn describe_counts(nodes: usize, triangles: usize) -> String {
    format!("{nodes} node{}  {triangles} triangle{}", plural(nodes), plural(triangles))
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Format a duration for the status bar without pretending to more precision
/// than is meaningful.
pub fn describe_elapsed(elapsed: std::time::Duration) -> String {
    let millis = elapsed.as_secs_f64() * 1000.0;
    if millis < 1000.0 {
        format!("{} ms", format_number(millis, 0))
    } else {
        format!("{} s", format_number(millis / 1000.0, 2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_is_read_in_the_display_unit() {
        assert_eq!(
            commit_param("1.8", ParamKind::Length { min: 0.0 }, Unit::Metre),
            Commit::Value(ParamValue::Length(1800.0))
        );
        assert_eq!(
            commit_param("1,8", ParamKind::Length { min: 0.0 }, Unit::Millimetre),
            Commit::Value(ParamValue::Length(1.8))
        );
    }

    #[test]
    fn garbage_reverts_rather_than_raising_a_dialog() {
        // Spec acceptance criterion 14.
        for bad in ["", "  ", "abc", "1.2.3", "--4", "NaN", "inf", "12mmm"] {
            assert_eq!(commit_param(bad, ParamKind::Length { min: 0.0 }, Unit::Millimetre), Commit::Revert, "{bad:?}");
            assert_eq!(commit_length(bad, Unit::Millimetre), None, "{bad:?}");
            assert_eq!(commit_angle(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_dimension_below_its_minimum_is_clamped_not_rejected() {
        let kind = ParamKind::Length { min: 1e-3 };
        assert_eq!(commit_param("0", kind, Unit::Millimetre), Commit::Value(ParamValue::Length(1e-3)));
        assert_eq!(commit_param("-5", kind, Unit::Millimetre), Commit::Value(ParamValue::Length(1e-3)));
        assert_eq!(commit_param("5", kind, Unit::Millimetre), Commit::Value(ParamValue::Length(5.0)));
    }

    #[test]
    fn a_count_is_rounded_and_clamped_to_its_range() {
        let kind = ParamKind::Count { min: 3, max: 128 };
        assert_eq!(commit_param("6", kind, Unit::Millimetre), Commit::Value(ParamValue::Count(6)));
        assert_eq!(commit_param("6.7", kind, Unit::Millimetre), Commit::Value(ParamValue::Count(7)));
        assert_eq!(commit_param("1", kind, Unit::Millimetre), Commit::Value(ParamValue::Count(3)));
        assert_eq!(commit_param("999", kind, Unit::Millimetre), Commit::Value(ParamValue::Count(128)));
        assert_eq!(commit_param("-4", kind, Unit::Millimetre), Commit::Value(ParamValue::Count(3)));
    }

    #[test]
    fn an_angle_is_clamped_to_its_range_and_stays_in_degrees() {
        let kind = ParamKind::Angle { min: 1.0, max: 360.0 };
        // Angles are always degrees regardless of the length unit.
        assert_eq!(commit_param("90", kind, Unit::Metre), Commit::Value(ParamValue::Angle(90.0)));
        assert_eq!(commit_param("999", kind, Unit::Millimetre), Commit::Value(ParamValue::Angle(360.0)));
        assert_eq!(commit_param("0", kind, Unit::Millimetre), Commit::Value(ParamValue::Angle(1.0)));
    }

    #[test]
    fn a_choice_index_cannot_run_off_the_end() {
        let kind = ParamKind::Choice { options: &["Across corners", "Across flats"] };
        assert_eq!(commit_param("1", kind, Unit::Millimetre), Commit::Value(ParamValue::Choice(1)));
        assert_eq!(commit_param("7", kind, Unit::Millimetre), Commit::Value(ParamValue::Choice(1)));
        assert_eq!(commit_param("-1", kind, Unit::Millimetre), Commit::Value(ParamValue::Choice(0)));
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
            assert_eq!(commit_param(&shown, kind, unit), Commit::Value(value), "{shown:?} in {unit:?}");
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
        let keymap = Keymap::default();
        let chord = chord_from_egui(egui::Key::S, egui::Modifiers::COMMAND);
        assert_eq!(keymap.command_for(&chord), Some(Command::Save));
        let chord = chord_from_egui(egui::Key::S, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
        assert_eq!(keymap.command_for(&chord), Some(Command::SaveAs));
        let chord = chord_from_egui(egui::Key::Delete, egui::Modifiers::NONE);
        assert_eq!(keymap.command_for(&chord), Some(Command::Delete));
        let chord = chord_from_egui(egui::Key::ArrowUp, egui::Modifiers::NONE);
        assert_eq!(keymap.command_for(&chord), Some(Command::NudgeUp));
    }

    #[test]
    fn every_default_binding_can_be_produced_by_a_real_key_press() {
        // A binding nobody can type would be a silent dead end, so check every
        // preset's key names against the toolkit's own key list.
        let names: Vec<&'static str> = egui::Key::ALL.iter().map(|k| k.name()).collect();
        for preset in scadstudio_core::keymap::Preset::ALL {
            let keymap = Keymap::from_preset(preset);
            for command in Command::ALL {
                let chord = keymap.binding(*command).unwrap();
                assert!(
                    names.contains(&chord.key.as_str()),
                    "{preset:?}: {:?} is bound to {:?}, which is not a key that exists",
                    command,
                    chord.key
                );
            }
        }
    }

    #[test]
    fn a_bounding_box_reads_as_three_dimensions_in_the_display_unit() {
        let size = scadstudio_geom::Vec3::new(40.0, 20.0, 4.0);
        assert_eq!(describe_size(size, Unit::Millimetre), "40 x 20 x 4 mm");
        assert_eq!(describe_size(size, Unit::Metre), "0.04 x 0.02 x 0.004 m");
    }

    #[test]
    fn counts_and_durations_read_naturally() {
        assert_eq!(describe_counts(1, 1), "1 node  1 triangle");
        assert_eq!(describe_counts(3, 240), "3 nodes  240 triangles");
        assert_eq!(describe_elapsed(std::time::Duration::from_millis(12)), "12 ms");
        assert_eq!(describe_elapsed(std::time::Duration::from_millis(2500)), "2.5 s");
    }
}

//! Shared interface pieces, with the decision-making parts kept as pure
//! functions so they can be tested without an egui context.

use simple3d_core::keymap::{Chord, Command, Keymap};
use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::unit::{format_angle, format_length, format_number, parse_entry, parse_entry_plain, Unit};
use std::collections::{HashMap, HashSet};

/// What a text field's content should do to the model when the user commits it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Commit {
    /// A new value to store.
    Value(ParamValue),
    /// Unparseable: keep the previous value and mark the field (spec section 4,
    /// acceptance criterion 14). The text the user typed stays where it is --
    /// clearing it would throw away the very thing they need to correct.
    Revert,
}

/// Interpret typed text for a parameter of the given kind. Out-of-range values
/// are clamped rather than rejected -- the user's intent is clear, and refusing a
/// number they can see in the field is more confusing than adjusting it.
///
/// `current` is what the parameter holds now, in stored terms (millimetres for
/// a length, degrees for an angle). It is needed because the field accepts a
/// *delta*: `+2` means "two more than whatever this is", and with several nodes
/// selected that resolves differently for each of them.
pub fn commit_param(text: &str, kind: ParamKind, unit: Unit, current: f64) -> Commit {
    // A length is the only kind a unit suffix means anything to; an angle typed
    // as "4 cm" would otherwise silently become forty degrees.
    let entry = match kind {
        ParamKind::Length { .. } => parse_entry(text, unit),
        _ => parse_entry_plain(text),
    };
    let Some(entry) = entry else { return Commit::Revert };
    let current_shown = match kind {
        ParamKind::Length { .. } => unit.from_mm(current),
        _ => current,
    };
    Commit::Value(value_from_display(kind, unit, entry.resolve(current_shown)))
}

/// Store a number expressed in the field's own display terms as a value of the
/// given kind, with the same clamping `commit_param` applies. The scrub gesture
/// goes through here rather than through text, so a drag and a typed number
/// cannot disagree about what is in range.
pub fn value_from_display(kind: ParamKind, unit: Unit, shown: f64) -> ParamValue {
    match kind {
        ParamKind::Length { min } => ParamValue::Length(unit.to_mm(shown).max(min)),
        ParamKind::Angle { min, max } => ParamValue::Angle(shown.clamp(min, max)),
        ParamKind::Count { min, max } => {
            if shown < 0.0 {
                ParamValue::Count(min)
            } else {
                ParamValue::Count((shown.round() as u32).clamp(min, max))
            }
        }
        ParamKind::Bool => ParamValue::Bool(shown != 0.0),
        ParamKind::Choice { options } => {
            ParamValue::Choice((shown.max(0.0) as u32).min(options.len().saturating_sub(1) as u32))
        }
    }
}

/// The stored number a parameter holds, for resolving a delta against.
pub fn param_number(value: ParamValue) -> f64 {
    match value {
        ParamValue::Length(mm) => mm,
        ParamValue::Angle(deg) => deg,
        ParamValue::Count(n) => n as f64,
        ParamValue::Choice(n) => n as f64,
        ParamValue::Bool(b) => b as u8 as f64,
    }
}

/// A position component, in stored millimetres. Positions may be negative, so
/// there is no minimum; `current` is what the field holds now, so `+2` and
/// `- 5` adjust it.
pub fn commit_length(text: &str, unit: Unit, current: f64) -> Option<f64> {
    let entry = parse_entry(text, unit)?;
    Some(unit.to_mm(entry.resolve(unit.from_mm(current))))
}

/// A rotation component in degrees. Rotations are free, since a node may be
/// turned any way.
pub fn commit_angle(text: &str, current: f64) -> Option<f64> {
    let entry = parse_entry_plain(text)?;
    Some(entry.resolve(current))
}

/// A scale factor. Unitless, and clamped to something that still produces a
/// solid: a factor of zero flattens a shape into a plane and a negative one
/// turns it inside out, and neither is a thing to hand a slicer.
pub fn commit_factor(text: &str, current: f64) -> Option<f64> {
    let entry = parse_entry_plain(text)?;
    Some(entry.resolve(current).max(simple3d_core::scene::Node::MIN_SCALE))
}

/// The em dash a field shows when the nodes it covers do not agree. Typing over
/// it applies to all of them; leaving it alone leaves each as it was.
pub const MIXED: &str = "\u{2014}";

/// What a field shows for a set of values: the value when they agree, and an em
/// dash when they do not.
pub fn shared_text(mut values: impl Iterator<Item = String>) -> String {
    let Some(first) = values.next() else { return String::new() };
    if values.all(|v| v == first) {
        first
    } else {
        MIXED.to_string()
    }
}

/// How much one horizontal pixel of a label scrub is worth.
///
/// Shift is fine, Ctrl is coarse -- the same two modifiers the manipulator uses
/// for the same two meanings, so there is one thing to learn rather than two.
pub fn scrub_step(step: f64, fine: bool, coarse: bool) -> f64 {
    let factor = match (fine, coarse) {
        (true, _) => 0.1,
        (false, true) => 10.0,
        _ => 1.0,
    };
    step * factor / PIXELS_PER_STEP
}

/// Pixels of drag per one step of the value. Slow enough that a value can be
/// landed on, fast enough that a field can be crossed.
pub const PIXELS_PER_STEP: f64 = 6.0;

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
///
/// It also remembers which fields were last given something unreadable. A
/// rejected field keeps the text that was typed into it and wears a red frame:
/// the value the model holds is untouched, and the thing the user has to
/// correct is still in front of them.
#[derive(Default)]
pub struct FieldBuffers {
    buffers: HashMap<egui::Id, String>,
    errors: HashSet<egui::Id>,
}

impl FieldBuffers {
    /// Draw a single-line field. Returns the committed text when the user
    /// presses Enter or leaves the field, and `None` while they are still typing.
    ///
    /// The value shown comes from the model whenever the field is not being
    /// edited, which is what makes a manipulator drag update the property editor
    /// live and typing move the handles -- one source of truth, both directions.
    pub fn field(&mut self, ui: &mut egui::Ui, id: egui::Id, current: &str) -> Option<String> {
        let mut text = self.buffers.get(&id).cloned().unwrap_or_else(|| current.to_string());
        let rejected = self.is_rejected(id);
        let response = ui
            .scope(|ui| {
                if rejected {
                    // The mark is the field's own frame, so it is impossible to
                    // read the number without also reading that it was refused.
                    let visuals = ui.visuals_mut();
                    visuals.extreme_bg_color = crate::theme::token::DANGER.gamma_multiply(0.16);
                    let danger = egui::Stroke::new(1.0_f32, crate::theme::token::DANGER);
                    visuals.widgets.inactive.bg_stroke = danger;
                    visuals.widgets.hovered.bg_stroke = danger;
                    visuals.widgets.active.bg_stroke = danger;
                    visuals.selection.stroke = danger;
                }
                ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .id(id)
                        .desired_width(f32::INFINITY)
                        // Tabular figures: a column of dimensions has to line up,
                        // and no digit may change width while a value is scrubbed.
                        .font(egui::TextStyle::Monospace)
                        .horizontal_align(egui::Align::RIGHT),
                )
            })
            .inner;
        let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        // Escape is the way out of everything else in this application, and it
        // is the way out of a half-typed value too: the buffer is dropped and
        // the model keeps what it had. egui surrenders focus on Escape, so
        // without this the abandoned text would be committed on the way out.
        let abandoned = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
        if abandoned {
            self.buffers.remove(&id);
            self.errors.remove(&id);
            return None;
        }
        if response.has_focus() && !entered {
            self.buffers.insert(id, text);
            return None;
        }
        if response.lost_focus() || entered {
            let committed = self.buffers.remove(&id).unwrap_or(text);
            if committed != current {
                return Some(committed);
            }
            self.errors.remove(&id);
            return None;
        }
        if response.changed() {
            self.buffers.insert(id, text);
        }
        None
    }

    /// The field took the value: drop any mark it was wearing.
    pub fn accept(&mut self, id: egui::Id) {
        self.errors.remove(&id);
    }

    /// The field's text could not be read. Put it back and mark it -- never
    /// clear it, and never touch the model.
    pub fn reject(&mut self, id: egui::Id, text: String) {
        self.buffers.insert(id, text);
        self.errors.insert(id);
    }

    /// Whether a field is currently wearing the mark.
    pub fn is_rejected(&self, id: egui::Id) -> bool {
        self.errors.contains(&id)
    }

    /// Forget every in-progress edit, for when the selection changes underneath.
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.errors.clear();
    }
}

/// A drag on a field's *label*, which scrubs the value.
///
/// The whole gesture is one undo step: the snapshot is taken when the drag
/// starts, and every frame after it only touches the scene. Forty snapshots for
/// one drag would make undo useless exactly where it is needed most. The id is
/// held so a pointer that runs off one label and over another cannot hand the
/// gesture to a field the user never grabbed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scrub {
    pub id: Option<egui::Id>,
}

/// This frame's pointer movement as a change in the field's own units.
pub fn scrub_delta(dx: f32, step: f64, fine: bool, coarse: bool) -> f64 {
    dx as f64 * scrub_step(step, fine, coarse)
}

/// The scrub increment for a field of a given kind, in the unit the field
/// shows. One millimetre in a millimetre document, one degree, one segment.
pub fn scrub_increment(kind: ParamKind, unit: Unit) -> f64 {
    match kind {
        ParamKind::Length { .. } => unit.from_mm(1.0),
        ParamKind::Angle { .. } => 1.0,
        _ => 1.0,
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
pub fn describe_size(size: simple3d_geom::Vec3, unit: Unit) -> String {
    format!(
        "{} x {} x {} {}",
        format_length(size.x, unit),
        format_length(size.y, unit),
        format_length(size.z, unit),
        unit.suffix()
    )
}

/// A point, in the display unit, for a message that names one.
pub fn describe_point(p: simple3d_geom::Vec3, unit: Unit) -> String {
    format!(
        "{}, {}, {} {}",
        format_length(p.x, unit),
        format_length(p.y, unit),
        format_length(p.z, unit),
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
        for preset in simple3d_core::keymap::Preset::ALL {
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
        let size = simple3d_geom::Vec3::new(40.0, 20.0, 4.0);
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

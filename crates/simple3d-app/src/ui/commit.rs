//! Reading what was typed into a field back into a parameter.

use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::unit::{format_angle, format_length, parse_entry, parse_entry_plain, wrap_degrees, Unit};

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

/// A rotation component in degrees. A node may be turned any way, so nothing is
/// refused -- but what comes back is the direction it ends up facing rather than
/// the number that was typed to get there: a turn is brought into `[0, 360)`, so
/// 400 is 40 and -90 is 270 (issue 84).
///
/// The relative forms are resolved before the wrap, not after, so `+1` on a
/// field showing 359 is a degree further round and reads 0.
pub fn commit_angle(text: &str, current: f64) -> Option<f64> {
    let entry = parse_entry_plain(text)?;
    Some(wrap_degrees(entry.resolve(current)))
}

/// A scale factor. Unitless, and clamped to something that still produces a
/// solid: a factor of zero flattens a shape into a plane and a negative one
/// turns it inside out, and neither is a thing to hand a slicer.
pub fn commit_factor(text: &str, current: f64) -> Option<f64> {
    let entry = parse_entry_plain(text)?;
    Some(entry.resolve(current).max(simple3d_core::scene::Node::MIN_SCALE))
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

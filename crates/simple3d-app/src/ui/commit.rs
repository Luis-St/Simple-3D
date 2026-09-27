//! Reading typed text back into a parameter.

use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::unit::{format_angle, format_length, parse_entry, parse_entry_plain, wrap_degrees, Unit};

/// What committing a text field should do to the model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Commit {
    /// A new value to store.
    Value(ParamValue),
    /// Unparseable: keep the old value and mark the field, leaving the typed text to correct (spec
    /// section 4, acceptance criterion 14).
    Revert,
}

/// Interpret typed text for a parameter kind, clamping out-of-range values. `current` is the stored
/// value, since deltas like `+2` resolve per selected node.
pub fn commit_param(text: &str, kind: ParamKind, unit: Unit, current: f64) -> Commit {
    // Only lengths take unit suffixes; "4 cm" must not become forty degrees.
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

/// Store a display-unit number as a value of this kind with `commit_param`'s clamping; scrubs go
/// through here so drags and typing agree on ranges.
pub fn value_from_display(kind: ParamKind, unit: Unit, shown: f64) -> ParamValue {
    match kind {
        ParamKind::Length { min } => ParamValue::Length(unit.to_mm(shown).max(min)),
        ParamKind::Angle { min, max, wrap } => {
            ParamValue::Angle(if wrap { wrap_degrees(shown) } else { shown.clamp(min, max) })
        }
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

/// The stored number a parameter holds, for resolving a delta.
pub fn param_number(value: ParamValue) -> f64 {
    match value {
        ParamValue::Length(mm) => mm,
        ParamValue::Angle(deg) => deg,
        ParamValue::Count(n) => n as f64,
        ParamValue::Choice(n) => n as f64,
        ParamValue::Bool(b) => b as u8 as f64,
    }
}

/// A position component in stored millimetres, unbounded; `current` resolves `+2` and `- 5`.
pub fn commit_length(text: &str, unit: Unit, current: f64) -> Option<f64> {
    let entry = parse_entry(text, unit)?;
    Some(unit.to_mm(entry.resolve(unit.from_mm(current))))
}

/// A rotation component in degrees, wrapped into `[0, 360)` (issue 84); relative forms resolve
/// before wrapping, so `+1` on 359 gives 0.
pub fn commit_angle(text: &str, current: f64) -> Option<f64> {
    let entry = parse_entry_plain(text)?;
    Some(wrap_degrees(entry.resolve(current)))
}

/// A scale factor, clamped positive, since zero flattens and negative inverts the solid.
pub fn commit_factor(text: &str, current: f64) -> Option<f64> {
    let entry = parse_entry_plain(text)?;
    Some(entry.resolve(current).max(simple3d_core::scene::Node::MIN_SCALE))
}

/// How a parameter's value is shown in its field.
pub fn show_param(value: ParamValue, unit: Unit) -> String {
    match value {
        ParamValue::Length(mm) => format_length(mm, unit),
        ParamValue::Angle(deg) => format_angle(deg),
        ParamValue::Count(n) => n.to_string(),
        ParamValue::Choice(n) => n.to_string(),
        ParamValue::Bool(b) => b.to_string(),
    }
}

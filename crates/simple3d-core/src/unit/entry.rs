//! Reading typed text back into a number.

use super::*;

/// A parsed numeric field entry: a number, expression, other-unit value, or a delta (`+2`) that
/// only the caller can resolve, per selected node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    /// The number, in the field's display unit.
    pub value: f64,
    /// True when `value` is added to the current value rather than replacing it.
    pub relative: bool,
}

impl Entry {
    /// Resolve against the field's current value, in the display unit.
    pub fn resolve(self, current: f64) -> f64 {
        if self.relative {
            current + self.value
        } else {
            self.value
        }
    }
}

/// Read a field's text in a display unit: absolute (`40`), expressions (`40/3`, `(2+3)*4`), unit
/// suffixes (`4 cm`) and deltas (`+2`, `+= 2`, `- 5`, `-= 5`). A leading `-` is a delta only when
/// followed by a space or `=`, so `-5` stays minus five.
pub fn parse_entry(text: &str, unit: Unit) -> Option<Entry> {
    parse_entry_in(text, Some(unit))
}

/// `parse_entry` for non-length fields (angles, counts), where unit suffixes convert nothing.
pub fn parse_entry_plain(text: &str) -> Option<Entry> {
    parse_entry_in(text, None)
}

pub(crate) fn parse_entry_in(text: &str, unit: Option<Unit>) -> Option<Entry> {
    let trimmed = text.trim();
    let (relative, negate, rest) = if let Some(rest) = trimmed.strip_prefix("+=") {
        (true, false, rest)
    } else if let Some(rest) = trimmed.strip_prefix("-=") {
        (true, true, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (true, false, rest)
    } else if let Some(rest) = trimmed.strip_prefix('-').filter(|r| r.starts_with(char::is_whitespace)) {
        (true, true, rest)
    } else {
        (false, false, trimmed)
    };
    let value = evaluate(rest, unit)?;
    Some(Entry { value: if negate { -value } else { value }, relative })
}

/// Parse a number with no unit context; suffixes are ignored, expressions work. `None` if
/// unparseable, and the caller restores and marks the field (spec section 4).
pub fn parse_number(text: &str) -> Option<f64> {
    evaluate(text.trim(), None)
}

/// Parse an absolute length in `unit` into millimetres; relative entries are refused, having
/// nothing to add to.
pub fn parse_length(text: &str, unit: Unit) -> Option<f64> {
    let entry = parse_entry(text, unit)?;
    (!entry.relative).then(|| unit.to_mm(entry.value))
}

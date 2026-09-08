//! Reading what was typed into a field back into a number.

use super::*;

/// What the user typed into a numeric field, once it has been read.
///
/// A field accepts more than a number: an expression (`40/3`), a value in a unit
/// other than the document's (`4 cm` in a millimetre drawing) and a *delta*
/// (`+2`, `- 5`) that adjusts whatever is already there. The delta is the reason
/// this is a struct rather than an `f64`: only the caller knows what "already
/// there" is, and with several nodes selected each of them has its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    /// The number, expressed in the unit the field is displayed in.
    pub value: f64,
    /// True when `value` is an adjustment to add to the current value rather
    /// than a replacement for it.
    pub relative: bool,
}

impl Entry {
    /// Resolve against what the field currently holds, in the display unit.
    pub fn resolve(self, current: f64) -> f64 {
        if self.relative {
            current + self.value
        } else {
            self.value
        }
    }
}

/// Read a field's text in the context of a display unit.
///
/// Absolute (`40`), expression (`40/3`, `12+8`, `(2+3)*4`), unit-suffixed
/// (`4 cm`, which is `40` in a millimetre document and `0.04` in a metre one)
/// and relative (`+2`, `+= 2`, `- 5`, `-= 5`).
///
/// **A leading `-` is only a delta when a space or an `=` follows it.** `-5` has
/// to keep meaning minus five, because a position field must be able to hold a
/// negative number, and no field can read the same six keystrokes two ways. `+`
/// has no such conflict, so a bare `+5` is a delta.
pub fn parse_entry(text: &str, unit: Unit) -> Option<Entry> {
    parse_entry_in(text, Some(unit))
}

/// `parse_entry` for a field that is not a length -- an angle, a count -- where
/// a unit suffix converts nothing because there is nothing to convert into.
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

/// Parse a number with no unit context: any suffix the user typed is accepted
/// and ignored, since there is nothing to convert it into. Expressions work
/// here too. `None` on anything unparseable -- callers restore the previous
/// value and mark the field rather than raising a dialog (spec section 4).
pub fn parse_number(text: &str) -> Option<f64> {
    evaluate(text.trim(), None)
}

/// Parse an absolute length typed in `unit` into stored millimetres. A relative
/// entry is refused here rather than silently read as an absolute one: the
/// caller that has no current value to add it to must not guess.
pub fn parse_length(text: &str, unit: Unit) -> Option<f64> {
    let entry = parse_entry(text, unit)?;
    (!entry.relative).then(|| unit.to_mm(entry.value))
}

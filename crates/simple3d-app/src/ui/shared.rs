//! What a field shows over a selection that does not agree.

use super::*;

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

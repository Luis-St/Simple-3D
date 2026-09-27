//! What a field shows over a selection that does not agree.

use super::*;

/// The shared value when all agree, else an em dash.
pub fn shared_text(mut values: impl Iterator<Item = String>) -> String {
    let Some(first) = values.next() else { return String::new() };
    if values.all(|v| v == first) {
        first
    } else {
        MIXED.to_string()
    }
}

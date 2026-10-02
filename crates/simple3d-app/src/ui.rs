//! Shared interface pieces; the decision logic is pure functions, testable without egui.

mod commit;
pub use commit::{
    commit_angle, commit_factor, commit_length, commit_param, param_number, show_param, value_from_display, Commit,
};
mod shared;
pub use shared::shared_text;
mod buffers;
pub use buffers::{Field, FieldBuffers, Scrubbed};
mod field;
pub(crate) use field::*;
mod scrub;
pub use scrub::{scrub_gesture, scrub_increment, Scrub};
mod menu;
pub use menu::{dialog_button, key_from_name, marked, menu_entry, menu_label};
mod chord;
pub use chord::{keys_down, ChordHold};
mod describe;
pub(crate) use describe::plural;
pub use describe::{describe_counts, describe_elapsed, describe_point, describe_size};
#[cfg(test)]
mod tests;

/// Shown when covered nodes disagree; typing applies to all, leaving it keeps each.
pub const MIXED: &str = "\u{2014}";

/// Drag pixels per value step: slow enough to land on a value, fast enough to cross a field.
pub const PIXELS_PER_STEP: f64 = 6.0;

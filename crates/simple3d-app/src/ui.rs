//! Shared interface pieces, with the decision-making parts kept as pure
//! functions so they can be tested without an egui context.

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
pub use menu::{dialog_button, key_from_name, menu_entry, menu_label};
mod chord;
pub use chord::{keys_down, ChordHold};
mod describe;
pub use describe::{describe_counts, describe_elapsed, describe_point, describe_size};
#[cfg(test)]
mod tests;

/// The em dash a field shows when the nodes it covers do not agree. Typing over
/// it applies to all of them; leaving it alone leaves each as it was.
pub const MIXED: &str = "\u{2014}";

/// Pixels of drag per one step of the value. Slow enough that a value can be
/// landed on, fast enough that a field can be crossed.
pub const PIXELS_PER_STEP: f64 = 6.0;

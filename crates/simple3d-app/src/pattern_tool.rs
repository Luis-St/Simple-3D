//! The custom pattern kind creation tool (issue 67).
//!
//! The six kinds a pattern ships with are the ones worth having a name for.
//! This is where a user makes their own: a rule built out of *stages*, each one
//! repeating whatever the stages before it made, named and kept on a shelf for
//! any other project.
//!
//! It edits the pattern node itself rather than a draft of one. A rule is only
//! ever judged by what it lays out, so every number typed here moves the copies
//! in the viewport at once, and every one of them is an ordinary parameter
//! edit -- which is why undo, the project file and the clipboard needed nothing
//! added for any of this.
//!
//! What is saved to the shelf is the *rule*, not the pattern: the stage numbers
//! and nothing else. A node still stores those numbers itself, so a project
//! opened on a machine that has never seen the kind still lays out correctly.
//!
//! It is an [in-place popup](crate::popup) rather than a dialog (issue 96). It
//! used to be a window most of the screen wide, half of it a second viewport
//! rendering the very same scene from a camera of its own -- a picture of the
//! pattern, in front of the picture of the pattern. What the tool is for is
//! seeing a rule take shape on the model, and the model is already on screen:
//! the window now floats over it, is dragged out of the way by its title bar,
//! rolls up to its bar alone, and leaves the viewport underneath free to be
//! orbited and zoomed while the numbers stay up. The preview is the viewport.
//!
//! Two of the things a rule needed most are here as well (issue 79): any of the
//! six fixed kinds can be used as the template a rule starts from, rather than
//! the tool throwing the layout away on the way in, and a stage says what it
//! *does* before it says its numbers, so a run offers a run's few fields
//! instead of all nine.
//!
//! And it is laid out as a builder rather than a form (issue 79). Each stage is
//! a card, each variation on a stage a card inside it, and the scatter the last
//! card of all; a stage, a variation and a part of the scatter are each *added*
//! as the thing they are, from a row of chips, and taken off by their own
//! cross. What is on screen is what the rule does, and nothing else.

mod body;
mod kinds;
mod open;
pub(crate) use body::*;
mod shelf;
pub(crate) use shelf::*;
mod stages;
pub(crate) use stages::*;
mod variation_card;
use variation_card::*;
mod cards;
pub(crate) use cards::*;
mod describe;
pub(crate) use describe::*;
mod actions;
pub(crate) use actions::*;

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "pattern-tool";

/// How wide the window is: an X, Y and Z field side by side inside a card
/// inside another, each still wide enough to read, and no wider. A popup lives
/// over the model, so every pixel of it is a pixel of the pattern being laid out
/// that cannot be seen.
const WIDTH: f32 = 430.0;

/// The cross that drops a stage: twice the height egui's small button comes out
/// at, and square, because it is a mark rather than a word and a wide one reads
/// as a button with its label missing.
const DROP_STAGE: f32 = 28.0;

/// The cross that takes a variation or a part of the scatter off: smaller than
/// a stage's, which throws away more.
const CARD_CROSS: f32 = 22.0;

/// How round a card's corners are.
const CARD_ROUNDING: f32 = 4.0;

/// The frame a card of the builder is drawn in: a stage, a variation on it, a
/// part of the scatter. Each nests in the one it belongs to, filled a shade
/// apart so the nesting reads without a heading having to say it.
pub(crate) fn card(fill: egui::Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, crate::theme::token::SURFACE_3))
        .corner_radius(CARD_ROUNDING)
        .inner_margin(egui::Margin::same(6))
}

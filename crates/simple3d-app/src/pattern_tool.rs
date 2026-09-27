//! The custom pattern kind creation tool (issue 67).
//!
//! Builds rules from stages, each repeating what the earlier stages made, and saves them as named
//! kinds. It edits the pattern node directly, so every number is an ordinary parameter edit (undo,
//! files and clipboard need nothing extra). The shelf stores only the rule; nodes keep their own
//! numbers.
//!
//! A non-modal [in-place popup](crate::popup) over the viewport, which is the preview (issue 96).
//! Fixed kinds serve as templates, stages pick a mode before their numbers, and the layout is a
//! builder of cards added from chips and removed by their crosses (issue 79).

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

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "pattern-tool";

/// The window width: X, Y and Z fields side by side in nested cards, still readable, no wider.
const WIDTH: f32 = 430.0;

/// The stage-dropping cross: square and twice egui's small button height, being a mark.
const DROP_STAGE: f32 = 28.0;

/// The cross removing a variation or scatter part, smaller than a stage's.
const CARD_CROSS: f32 = 22.0;

/// How round a card's corners are.
const CARD_ROUNDING: f32 = 4.0;

/// A builder card's frame (stage, variation or scatter part), each nested card a shade apart.
pub(crate) fn card(fill: egui::Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, crate::theme::token::SURFACE_3))
        .corner_radius(CARD_ROUNDING)
        .inner_margin(egui::Margin::same(6))
}

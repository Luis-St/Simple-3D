//! The custom pattern kind creation tool (issue 67).
//!
//! The six kinds a pattern ships with are the ones worth having a name for.
//! This is where a user makes their own: a rule built out of *stages*, each one
//! repeating whatever the stages before it made, named and kept on a shelf for
//! any other project.
//!
//! It edits the pattern node itself rather than a draft of one. A rule is only
//! ever judged by what it lays out, so every number typed here moves the copies
//! in the viewport behind the window at once, and every one of them is an
//! ordinary parameter edit -- which is why undo, the project file and the
//! clipboard needed nothing added for any of this.
//!
//! What is saved to the shelf is the *rule*, not the pattern: the stage numbers
//! and nothing else. A node still stores those numbers itself, so a project
//! opened on a machine that has never seen the kind still lays out correctly.

mod body;
mod kinds;
mod open;
pub(crate) use body::*;
mod stages;
pub(crate) use stages::*;
mod preview;
pub(crate) use preview::*;
mod actions;
pub(crate) use actions::*;

/// The cross that drops a stage: twice the height egui's small button comes out
/// at, and square, because it is a mark rather than a word and a wide one reads
/// as a button with its label missing.
const DROP_STAGE: f32 = 28.0;

/// The narrowest the stages are worth drawing at. Their rows are the property
/// panel's own, which stack a name above its field rather than beside it once
/// the room runs out, so the numbers stay usable well below the width two
/// columns need.
const STAGES_MIN: f32 = 240.0;

/// The widest the divider will drag them. Past this a stage row is a name, a
/// field and a stretch of nothing between the two, and the picture is paying
/// for it.
const STAGES_MAX: f32 = 560.0;

/// The smallest the preview can be and still be a viewport rather than a stamp.
const PREVIEW_MIN: f32 = 220.0;

/// The longest side the preview's image is rasterized at, whatever size it is
/// drawn at. See `paint_preview`.
const PREVIEW_MAX_PX: f32 = 1280.0;

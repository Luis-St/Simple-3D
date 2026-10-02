//! On-screen manipulators (spec section 6.2): move, rotate and resize.
//!
//! **Resize writes dimensions, never a scale factor**: dragging a box's right face changes its
//! width and leaves the left face in place. Which parameter drives which extent comes from the
//! registry's `axes` declaration, so handles appear only on governed axes.
//!
//! | Modifier | Effect |
//! |---|---|
//! | none  | snap to the increment (the scene step for move and resize, 15 degrees for rotate) |
//! | Alt   | drag freely, no snapping |
//! | Shift | snap coarsely (ten times the increment) |
//! | Ctrl  | resize about the centre (faces) / preserve proportions (corners) |
//!
//! Where Ctrl is also the snap key, a face drag with it snaps instead (`App::snap_holds_ctrl`).

mod mode;
pub use mode::{Handle, Mode};
mod mods;
pub use mods::Mods;
mod build;
pub use build::Gizmo;
mod hit;
pub use hit::CORNERS;
pub(crate) use hit::*;
mod drag;
pub use drag::Drag;
mod axis;
mod drag_carry;
mod drag_resize;
mod drag_update;
pub(crate) use axis::*;
pub use axis::{axis_colour, axis_name, axis_screen_sign, screen_aligned_axes};
mod nudge;
pub use nudge::{apply_nudge, Nudge};
mod pointer;
pub use pointer::{drag_phase, DragPhase, PointerState};
#[cfg(test)]
mod tests;

/// An axis arrow's length in screen pixels; handles keep a constant size (spec section 6.2).
pub const ARM_PIXELS: f64 = 78.0;

/// Where a plane handle's corner sits along each of its two axes.
pub const PLANE_FRACTION: f64 = 0.42;

/// The fraction of face-on area a plane handle must show to be drawn and grabbed (about twelve
/// degrees from edge-on).
pub const PLANE_MIN_FACING: f32 = 0.2;

/// Click tolerance in pixels.
pub const GRAB_PIXELS: f32 = 9.0;

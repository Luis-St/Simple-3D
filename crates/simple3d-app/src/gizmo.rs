//! On-screen manipulators (spec section 6.2): move, rotate and resize.
//!
//! The important property, and the one the tests are built around, is that
//! **resize writes dimensions, never a scale factor**. Dragging the right face
//! of a box changes its width parameter and leaves the left face where it was;
//! the property editor then shows the new real width, and the saved project file
//! contains no scale anywhere. Which parameter governs which extent comes from
//! the primitive registry's `axes` declaration, so a handle is only ever offered
//! on an axis some parameter genuinely controls.
//!
//! Modifiers, consistent across all three modes:
//!
//! | Modifier | Effect |
//! |---|---|
//! | none  | snap to the increment (the scene step for move and resize, 15 degrees for rotate) |
//! | Alt   | drag freely, no snapping |
//! | Shift | snap coarsely (ten times the increment) |
//! | Ctrl  | resize about the centre (faces) / preserve proportions (corners) |

mod mode;
pub use mode::{Handle, Mode};
mod mods;
pub use mods::Mods;
mod build;
pub use build::Gizmo;
mod hit;
pub(crate) use hit::*;
pub use hit::{get_axis, set_axis, CORNERS};
mod drag;
pub use drag::Drag;
mod axis;
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

/// How far from the origin an axis arrow reaches, in screen pixels. Handles keep
/// a constant on-screen size regardless of zoom (spec section 6.2).
pub const ARM_PIXELS: f64 = 78.0;

/// Where a plane handle's corner sits along each of its two axes.
pub const PLANE_FRACTION: f64 = 0.42;

/// Click tolerance in pixels.
pub const GRAB_PIXELS: f32 = 9.0;

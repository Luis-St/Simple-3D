//! The section plane: where it stands, how it is moved, and the window its
//! settings live in (issues 71, 72).
//!
//! The cut itself is the renderer's -- [`simple3d_geom::section`] decides what
//! survives the plane and closes what it opens. What is left is the half a
//! picture cannot show on its own: *where the plane is standing*, especially
//! when it has been slid past the model and is cutting nothing at all, and a
//! way of moving it by hand.
//!
//! Both are one thing here: a frame drawn in the plane, around the model, with
//! grips on it that slide the plane along its own axis. It is drawn with the 2D
//! painter over the finished image rather than as geometry in the frame, and
//! that is correct rather than convenient: everything the section keeps is
//! *behind* the plane, so there is nothing the frame could be hidden by.
//!
//! There are five of those grips -- the middle of each of the frame's four
//! edges, and the middle of the plane -- because which of them is in reach
//! depends entirely on where the model has been orbited to (issue 72). One grip
//! on the upper edge is behind the shape as soon as the plane is looked at from
//! below, and a control that has to be orbited to before it can be used is one
//! that has to be found again every time.
//!
//! The settings are an [in-place popup](crate::popup) over the viewport rather
//! than a section of the properties panel: the plane is not part of the
//! selection, and the numbers that say where it stands belong beside the frame
//! they move rather than in a panel that is describing something else. The
//! window standing open *is* the section being on -- closing it puts the model
//! back together, and rolling it up by its chevron leaves the cut where it is.
//!
//! Moving the plane is not an edit. It changes no geometry, so there is no undo
//! step for it and the document is not made dirty by it, exactly as toggling
//! the grid or an axis is not an edit either.

mod grips;
pub(crate) use grips::*;
pub use grips::{frame, grips, hash_section};
mod interact;
pub use interact::interact;
mod draw;
pub use draw::draw;
mod window;
pub(crate) use window::*;
pub use window::{middle_of, readout};
#[cfg(test)]
mod tests;

/// How much wider than the model the frame is drawn, so it reads as a plane
/// passing through the shape rather than as an outline of it.
const MARGIN: f64 = 0.2;

/// The smallest half-width the frame is drawn at, in millimetres: a section
/// through a 2 mm pin still needs something to grab.
const MIN_HALF: f64 = 10.0;

/// What the frame falls back to with nothing in the scene: a square about the
/// origin, big enough to see and to take hold of.
const EMPTY_HALF: f64 = 25.0;

/// How big the grip is on screen, in points.
const GRIP: f32 = 13.0;

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "section-tool";

/// How wide the window is: three axis chips across it, the offset field and its
/// label, and no wider. A popup lives over the model, so every pixel of it is a
/// pixel of the thing being cut that cannot be seen.
const WIDTH: f32 = 300.0;

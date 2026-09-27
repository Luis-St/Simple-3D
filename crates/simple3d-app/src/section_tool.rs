//! The section plane: where it stands, how it is moved, and its window (issues 71, 72).
//!
//! The renderer does the cut ([`simple3d_geom::section`]). This shows where the plane stands with a
//! frame drawn in it, painted in 2D over the image since nothing kept can be in front of it. Five
//! grips (four edge middles and the centre) slide the plane, so one is reachable from any orbit
//! (issue 72). The settings are an [in-place popup](crate::popup); its window being open is the
//! section being on. Moving the plane is not an edit: no undo step, no dirty document.

mod grips;
pub(crate) use grips::*;
pub use grips::{frame, grips, hash_section};
mod interact;
pub use interact::interact;
mod draw;
pub use draw::draw;
mod crossing;
pub use crossing::cut;
mod window;
pub(crate) use window::*;
pub use window::{middle_of, readout};
#[cfg(test)]
mod tests;

/// How much wider than the model the frame is drawn, so it reads as a plane through the shape.
const MARGIN: f64 = 0.2;

/// The frame's smallest half-width in millimetres, so a tiny part still has something to grab.
const MIN_HALF: f64 = 10.0;

/// The frame's half-width with nothing in the scene.
const EMPTY_HALF: f64 = 25.0;

/// The grip's on-screen size, in points.
const GRIP: f32 = 13.0;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "section-tool";

/// The window width: three axis chips, the offset field and its label, no wider.
const WIDTH: f32 = 300.0;

//! The viewport panel: navigation, picking, the manipulator overlay and the
//! bounding-box readout (spec sections 6.1, 6.2).
//!
//! The shaded image comes from the software rasterizer as a texture; the
//! manipulator, the bounding box and the drag readout are drawn on top with the
//! toolkit's 2D painter, so they are always visible and can be hovered.

mod show;
pub use show::show;
pub(crate) use show::*;
mod paint;
pub(crate) use paint::*;
mod gesture;
pub(crate) use gesture::*;
pub use gesture::{apply_gesture, nav_gesture, Gesture};
mod navigate;
pub use navigate::apply_zoom;
pub(crate) use navigate::*;
mod manipulate;
pub(crate) use manipulate::*;
mod pattern_grips;
pub(crate) use pattern_grips::*;
mod cursor;
pub use cursor::slide_cursor;
pub(crate) use cursor::*;
mod select;
pub(crate) use select::*;
mod overlays;
pub(crate) use overlays::*;
mod measure_overlay;
pub(crate) use measure_overlay::*;
mod view_cube;
pub(crate) use view_cube::*;
#[cfg(test)]
mod tests;

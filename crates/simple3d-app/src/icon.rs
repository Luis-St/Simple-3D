//! Geometric line icons, drawn rather than loaded.
//!
//! A single self-contained binary is a hard constraint, so there is no icon
//! font and no SVG loader here: every glyph is a handful of strokes in a unit
//! square, scaled into whatever rectangle it is asked to fill. That also keeps
//! them crisp at fractional DPI scaling, which a bitmap sheet would not.
//!
//! The house style is the design's: 16 px, 1.5 px stroke, geometric, and no
//! filled pictograms except the object-type glyphs in the outliner.

mod glyph;
pub use glyph::Glyph;
mod pen;
pub use pen::draw;
pub(crate) use pen::*;
mod paint;
pub(crate) use paint::*;
mod button;
pub use button::{button, button_sensing, shared_icon};
mod app_icon;
pub use app_icon::app_icon;
#[cfg(test)]
mod tests;

const TAU: f32 = std::f32::consts::TAU;

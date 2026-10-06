//! Geometric line icons, drawn as strokes in a unit square: no icon font or SVG loader in a single
//! binary, and crisp at fractional DPI. House style: 16 px, 1.5 px stroke, filled only for outliner
//! object-type glyphs.

mod glyph;
pub use glyph::Glyph;
mod pen;
pub use pen::draw;
pub(crate) use pen::*;
mod paint;
mod paint_tools;
pub(crate) use paint::*;
mod button;
pub use button::{button, button_sensing, shared_icon};
mod app_icon;
pub use app_icon::app_icon;
#[cfg(test)]
mod tests;

const TAU: f32 = std::f32::consts::TAU;

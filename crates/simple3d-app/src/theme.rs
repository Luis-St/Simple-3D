//! The visual language: one palette, one set of metrics, applied once at
//! startup so no widget has to name a colour of its own.
//!
//! The tokens below are the whole vocabulary. Amber is selection, cyan is
//! measurement, red is destruction -- and none of the three collides with the
//! X/Y/Z axis colours, which is the reason selection is not blue like every
//! other modeller's.

mod palette;
pub use palette::{font, metric, token, PAINT_PRESETS};
mod text;
pub use text::{header_text, hint, list_scroll_area, numeric, value};
mod controls;
pub use controls::{axis_chip, axis_colour, choice, panel_header, toggle, twisty, AXIS_CHIP_WIDTH};
mod apply;
pub use apply::apply;
#[cfg(test)]
mod tests;

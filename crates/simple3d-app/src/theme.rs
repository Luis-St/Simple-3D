//! The visual language: one palette and one set of metrics applied at startup. Amber is selection, cyan
//! measurement, red destruction; selection is not blue so it never collides with the axis colours.

mod palette;
pub use palette::{font, metric, token, PAINT_PRESETS};
mod text;
pub use text::{header_text, hint, list_scroll_area, numeric, value};
mod controls;
pub use controls::{axes_chip, axis_chip, axis_colour, choice, panel_header, toggle, twisty, AXIS_CHIP_WIDTH};
mod apply;
pub use apply::apply;
#[cfg(test)]
mod tests;

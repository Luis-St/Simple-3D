//! The left dock: the outliner (spec section 7.2) over the primitive palette.
//!
//! The outliner is the centre of gravity of the application, so it gets the
//! dock's height and the palette gets a fixed strip under it. One row per node,
//! 22 px, and the marks that matter -- visibility, the boolean operator, a
//! failure -- are drawn on every row rather than revealed on hover, because a
//! mark you have to go looking for cannot be scanned.

mod tree;
pub use tree::{show_inside, visible_rows};
mod row;
pub(crate) use row::*;
mod row_parts;
pub use row_parts::operator_badge;
pub(crate) use row_parts::*;
mod drag;
pub(crate) use drag::*;
pub use drag::{drop_is_legal, drop_position, gap_line_y};
mod selection;
pub(crate) use selection::*;
mod context_menu;
pub(crate) use context_menu::*;
#[cfg(test)]
mod tests;

//! The left dock: the outliner (spec section 7.2) over the primitive palette. One 22 px row per node,
//! with visibility, operator and failure marks always drawn, not revealed on hover, so they can be scanned.

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

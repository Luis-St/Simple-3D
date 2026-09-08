//! The two docks and what moves between them.
//!
//! A panel is a header bar and a body. The header is the whole of its interface:
//! click it to roll the panel up to that bar, drag it to put it in the other
//! dock or somewhere else in this one. There is no separate arrange mode and no
//! menu of positions -- the thing you want to move is the thing you drag.
//!
//! Where each panel *is* lives in `AppSettings::layout`, beside the dock widths
//! that were already persisted there, so an arrangement survives a restart.

mod show;
pub use show::show;
mod header;
pub(crate) use header::*;
mod drag;
pub use drag::{reset, resolve_drag};
#[cfg(test)]
mod tests;

use simple3d_core::config::{Panel, Side};

/// A panel being dragged, and where it would land if it were dropped now.
#[derive(Clone, Copy, Debug, Default)]
pub struct DockDrag {
    pub panel: Option<Panel>,
    pub target: Option<(Side, usize)>,
}

/// Where a pointer at `y` would drop a panel in a dock whose header bars are at
/// `headers` (each the vertical centre of one header, in order).
///
/// Split out from the drawing so the rule can be reasoned about on its own: the
/// drop goes above every header the pointer is above, which is the index of the
/// first header below it.
pub fn drop_index(headers: &[f32], y: f32) -> usize {
    headers.iter().filter(|centre| **centre < y).count()
}

/// The id of a panel's header bar: the grip that is clicked to roll the panel up
/// and dragged to move it.
pub fn header_id(panel: Panel) -> egui::Id {
    egui::Id::new(("dock-header", panel))
}

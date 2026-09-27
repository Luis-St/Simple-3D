//! The two docks and what moves between them.
//!
//! A panel's header is its whole interface: click to roll it up, drag to move it within or between
//! docks. Positions live in `AppSettings::layout`, so arrangements survive restarts.

mod show;
pub use show::show;
mod header;
pub(crate) use header::*;
mod drag;
pub use drag::{reset, resolve_drag};
#[cfg(test)]
mod tests;

use simple3d_core::config::{Panel, Side};

/// A panel being dragged, and where it would land now.
#[derive(Clone, Copy, Debug, Default)]
pub struct DockDrag {
    pub panel: Option<Panel>,
    pub target: Option<(Side, usize)>,
}

/// Where a pointer at `y` drops a panel among header centres `headers`: before the first header
/// below it.
pub fn drop_index(headers: &[f32], y: f32) -> usize {
    headers.iter().filter(|centre| **centre < y).count()
}

/// A panel header's id: the grip clicked to roll up and dragged to move.
pub fn header_id(panel: Panel) -> egui::Id {
    egui::Id::new(("dock-header", panel))
}

//! Keeping a popup inside the window it belongs to.

use super::*;

/// Where the window rests this frame: where it was left, brought inside the viewport. Opens top
/// right, clear of the model and view cube. Clamped every frame (the viewport can shrink) against
/// the whole window, so rolling up moves nothing and the bar stays under the chevron.
pub(crate) fn settle(placement: &Placement, width: f32, bounds: egui::Rect) -> egui::Pos2 {
    let pos = placement.pos.unwrap_or_else(|| default_pos(width, bounds));
    let tall = if placement.collapsed { TITLE_BAR } else { placement.height.max(TITLE_BAR) };
    clamp_into(pos, egui::vec2(width, tall), bounds)
}

/// Where a never-moved popup opens: its top right a margin in from the viewport's.
pub(crate) fn default_pos(width: f32, bounds: egui::Rect) -> egui::Pos2 {
    const MARGIN: f32 = 16.0;
    bounds.right_top() + egui::vec2(-width - MARGIN, MARGIN)
}

/// Keep a window's title bar inside `bounds`, so it can be grabbed again; only the bar, or tall
/// popups would fight the drag.
pub(crate) fn clamp_into(pos: egui::Pos2, bar: egui::Vec2, bounds: egui::Rect) -> egui::Pos2 {
    let max_x = (bounds.right() - bar.x).max(bounds.left());
    let max_y = (bounds.bottom() - bar.y).max(bounds.top());
    egui::pos2(pos.x.clamp(bounds.left(), max_x), pos.y.clamp(bounds.top(), max_y))
}

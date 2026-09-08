//! Keeping a popup inside the window it belongs to.

use super::*;

/// Where the window comes to rest this frame: where it was left, brought inside
/// the viewport.
///
/// Opened in the top-left of the viewport, a comfortable margin in -- over the
/// corner of the picture rather than over the middle of it, which is where the
/// thing being worked on is.
///
/// Clamped every frame and not only when it is dragged, because the viewport
/// can be made smaller and a popup left off the edge of a shrunken one cannot
/// be reached to be dragged back. Clamped against the *whole* window rather
/// than its title bar alone, so the position that gets stored is one the window
/// already fits at -- rolling it up then loosens the clamp and moves nothing,
/// which is what a roll-up has to do: the bar stays exactly under the chevron
/// that was clicked.
pub(crate) fn settle(placement: &Placement, width: f32, bounds: egui::Rect) -> egui::Pos2 {
    let pos = placement.pos.unwrap_or_else(|| bounds.left_top() + egui::vec2(16.0, 16.0));
    let tall = if placement.collapsed { TITLE_BAR } else { placement.height.max(TITLE_BAR) };
    clamp_into(pos, egui::vec2(width, tall), bounds)
}

/// Keep a window's title bar inside `bounds`, so it can always be grabbed
/// again. Only the bar is kept in: a tall popup near the bottom edge would
/// otherwise be shoved upward every frame, which fights the drag.
pub(crate) fn clamp_into(pos: egui::Pos2, bar: egui::Vec2, bounds: egui::Rect) -> egui::Pos2 {
    let max_x = (bounds.right() - bar.x).max(bounds.left());
    let max_y = (bounds.bottom() - bar.y).max(bounds.top());
    egui::pos2(pos.x.clamp(bounds.left(), max_x), pos.y.clamp(bounds.top(), max_y))
}

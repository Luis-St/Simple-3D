//! The viewport's clicks once the objects are chosen: picking the edge, or drawing the line.

use super::*;
use crate::app::{App, Status};
use crate::view::View;

/// Whether the tool takes the viewport's clicks: while an edge or line is wanted. While choosing the
/// objects the viewport selects as usual.
pub(crate) fn wants_pointer(app: &App) -> bool {
    app.mirror_tool.as_ref().is_some_and(|tool| !tool.choosing && tool.by != MirrorBy::Axis)
}

/// A click picks the edge or puts the line's next end down; a right-click (not a right drag, which
/// orbits) lets the line go.
pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        if let Some(tool) = app.mirror_tool.as_mut() {
            tool.ends = [None, None];
        }
        return;
    }
    if !response.clicked_by(egui::PointerButton::Primary) {
        return;
    }
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else { return };
    let Some(by) = app.mirror_tool.as_ref().map(|tool| tool.by) else { return };
    match by {
        MirrorBy::Edge => match app.nearest_model_edge(view, cursor) {
            Some((a, b)) => {
                if let Some(tool) = app.mirror_tool.as_mut() {
                    tool.ends = [Some(a), Some(b)];
                }
            }
            None => app.status = Status::Info("Click on a body's edge to mirror across it".into()),
        },
        MirrorBy::Line => {
            let Some((point, _)) = app.pick_position(view, cursor, &[]) else { return };
            let Some(tool) = app.mirror_tool.as_mut() else { return };
            tool.ends = match tool.ends {
                [Some(start), None] => [Some(start), Some(point.at)],
                // A third click starts a new line.
                _ => [Some(point.at), None],
            };
        }
        MirrorBy::Axis => {}
    }
}

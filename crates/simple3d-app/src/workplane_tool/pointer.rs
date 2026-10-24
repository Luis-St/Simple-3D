//! The viewport's clicks while the tool is out: picking the face the workplane goes on.

use crate::app::{App, Status};
use crate::view::View;

/// Whether the tool takes the viewport's clicks: always while it is out.
pub(crate) fn wants_pointer(app: &App) -> bool {
    app.workplane_tool.is_some()
}

/// A click puts the workplane on the face under it, another click on another one, keeping what the
/// window set. The face is found as the cut tool finds its own: a flat face through its middle, a
/// facet of a curved one at the clicked point, facing the eye.
pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if !response.clicked_by(egui::PointerButton::Primary) {
        return;
    }
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else { return };
    match crate::cut_tool::face_under(app, view, cursor) {
        Some(face) => {
            if let Some(tool) = app.workplane_tool.as_mut() {
                tool.pick(face);
            }
        }
        None => app.status = Status::Info("Click a face of the model to put the workplane on".into()),
    }
}

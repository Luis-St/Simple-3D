//! The viewport's clicks while the tool is out: picking the face to cut along.

use super::*;
use crate::app::{App, Status};
use crate::view::View;

/// Whether the tool takes the viewport's clicks: always while it is out.
pub(crate) fn wants_pointer(app: &App) -> bool {
    app.cut_tool.is_some()
}

/// A click picks the face, another click another one; a right-click (not a right drag, which orbits)
/// lets it go.
pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        if let Some(tool) = app.cut_tool.as_mut() {
            tool.face = None;
        }
        return;
    }
    if !response.clicked_by(egui::PointerButton::Primary) {
        return;
    }
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else { return };
    match face_under(app, view, cursor) {
        Some(face) => {
            if let Some(tool) = app.cut_tool.as_mut() {
                tool.face = Some(face);
            }
        }
        None => app.status = Status::Info("Click a face of the model to cut along".into()),
    }
}

/// The plane of the face under `cursor`, through its middle and facing the eye's side: a flat face
/// as a whole, a facet of a curved one at the clicked point.
pub(crate) fn face_under(app: &App, view: &View, cursor: egui::Pos2) -> Option<CutPlane> {
    let (at, triangle, normal) = app.surface_hit_under(view, cursor, &[])?;
    let face = simple3d_geom::push_pull::flat_face(&app.evaluated.mesh, triangle).filter(|face| !face.curved);
    Some(match face {
        Some(face) => {
            let normal = if face.normal.dot(normal) < 0.0 { face.normal * -1.0 } else { face.normal };
            CutPlane { point: face.centre, normal: normal.normalized() }
        }
        None => CutPlane { point: at, normal: normal.normalized() },
    })
}

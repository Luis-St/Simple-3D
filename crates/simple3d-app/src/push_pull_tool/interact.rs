//! The tool's pointer: press on a face, drag along its normal, release to commit.

use super::*;
use crate::app::{App, Status};
use crate::view::View;
use simple3d_core::unit::format_length;

/// Fewer screen pixels than this per millimetre of the normal and the face is seen edge-on to its
/// travel: the drag then reads the pointer's height on the screen instead.
const EDGE_ON: f32 = 0.05;

/// The tool's pointer for one frame. Returns whether it took the pointer, so a release does not
/// also select what is under it.
pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) -> bool {
    if app.push_pull.drag.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.push_pull.drag = None;
        app.status = Status::Info("Push / pull cancelled".into());
        return true;
    }
    let cursor = ui.input(|i| i.pointer.hover_pos());
    if app.push_pull.drag.is_none() && response.drag_started_by(egui::PointerButton::Primary) {
        let press = ui.input(|i| i.pointer.press_origin()).or(cursor);
        if let Some(press) = press {
            match app.face_under(view, press) {
                Some((Ok(hit), grab)) => {
                    app.push_pull.drag = Some(PushDrag { hit, grab, press, distance: 0.0, prism: None })
                }
                Some((Err(why), _)) => app.status = Status::Warning(why.into()),
                None => {}
            }
        }
    }
    if let Some(drag) = app.push_pull.drag.as_mut() {
        if let Some(cursor) = cursor {
            let raw = travel(view, drag, cursor);
            let step = app.scene.settings.snap_step.max(1e-6);
            drag.distance = (raw / step).round() * step;
            drag.sweep();
        }
        let released = response.drag_stopped() || ui.input(|i| i.pointer.any_released());
        if released {
            let drag = app.push_pull.drag.take().expect("checked above");
            commit(app, drag);
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        return true;
    }
    if response.hovered() {
        if let Some((Ok(_), _)) = cursor.and_then(|cursor| app.face_under(view, cursor)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
    false
}

/// How far along the face's normal the pointer has dragged it, in millimetres.
fn travel(view: &View, drag: &PushDrag, cursor: egui::Pos2) -> f64 {
    let normal = drag.hit.captured.normal;
    let ends = (view.project(drag.grab), view.project(drag.grab + normal));
    let (Some((from, _)), Some((to, _))) = ends else { return drag.distance };
    let along = to - from;
    let moved = cursor - drag.press;
    if along.length() < EDGE_ON * view.pixels_per_mm() as f32 {
        // Looking straight down the normal: up the screen is out of the face.
        return -(moved.y as f64) / view.pixels_per_mm();
    }
    (moved.dot(along) / along.length_sq()) as f64
}

/// Add the dragged face's extrusion or reduction to its object, as one undo step.
pub(crate) fn commit(app: &mut App, drag: PushDrag) {
    if drag.distance.abs() < 1e-9 {
        app.status = Status::Info("Push / pull: drag the face out to add, or in to cut away".into());
        return;
    }
    // Tried on a copy, so a refusal leaves no empty undo step behind.
    let mut scene = app.scene.clone();
    let Some((holder, _)) =
        scene.push_pull(drag.hit.owner, &drag.hit.captured, drag.distance, &app.evaluated.node_frames)
    else {
        app.status = Status::Warning("That face could not be pushed or pulled".into());
        return;
    };
    let pushed = drag.distance > 0.0;
    app.edit(if pushed { "Push face" } else { "Pull face" }, None);
    app.scene = scene;
    app.select_only(holder);
    let unit = app.unit();
    let owner = app.scene.get(drag.hit.owner).map_or(String::new(), |node| node.name.clone());
    app.status = Status::Info(format!(
        "{} {owner}'s face by {} {}",
        if pushed { "Pushed" } else { "Pulled" },
        format_length(drag.distance.abs(), unit),
        unit.suffix()
    ));
}

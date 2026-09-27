//! Driving the manipulator from the pointer.

use super::*;
use crate::app::App;
use crate::gizmo::{self};
use crate::view::View;

/// The manipulator: hover, drag start and run, and Escape cancel. Returns whether the pointer is
/// the manipulator's, so a handle click does not also reselect.
pub(crate) fn manipulate(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) -> bool {
    // Whether a move drag snaps this frame (issue 68), read every frame so mode or key changes apply at once.
    app.snap_requested = app.geometry_snap_wanted(|k| ui.input(|i| i.key_down(k)), ui.input(|i| i.modifiers));
    let Some(id) = app.primary() else {
        app.drag = None;
        app.hover_handle = None;
        app.grabbed = None;
        return false;
    };
    let Some(gizmo) = app.gizmo_for(id) else {
        app.hover_handle = None;
        app.grabbed = None;
        return false;
    };
    // A split counts as a group: it has no dimensions, and its pieces are meshes.
    let is_group = app.scene.node(id).is_group() || app.scene.node(id).is_split();
    let cursor = ui.input(|i| i.pointer.hover_pos());
    let dragging = app.drag.is_some();

    // Hover hit-testing only matters while not dragging.
    if !dragging {
        app.hover_handle = cursor.and_then(|cursor| gizmo.hit_test(view, cursor, is_group));
    }

    // Remember what was under the pointer at press: a drag starts only past the threshold, by which
    // time the pointer has often left the 9 px handle.
    let (pressed, press_origin) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.press_origin()));
    if pressed {
        app.grabbed = press_origin.and_then(|at| gizmo.hit_test(view, at, is_group));
    }

    // All the pointer facts for this frame, read in one place.
    let pointer = gizmo::PointerState {
        escape: ui.input(|i| i.key_pressed(egui::Key::Escape)),
        released: response.drag_stopped() || ui.input(|i| i.pointer.any_released()),
        started: response.drag_started_by(egui::PointerButton::Primary),
        on_handle: app.grabbed.is_some(),
        have_cursor: cursor.is_some(),
    };
    let phase = gizmo::drag_phase(dragging, pointer);
    // Measured from the press point, not where the pointer slipped to before the drag began.
    let from = if phase == gizmo::DragPhase::Begin { press_origin.or(cursor) } else { cursor };
    app.manipulate_step(&gizmo, view, id, phase, app.grabbed, from, mods_from(ui));

    let owned = app.drag.is_some() || app.grabbed.is_some();
    if pointer.released {
        app.grabbed = None;
    }
    owned
}

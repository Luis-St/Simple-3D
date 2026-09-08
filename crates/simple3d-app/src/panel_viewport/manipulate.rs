//! Driving the manipulator from the pointer.

use super::*;
use crate::app::App;
use crate::gizmo::{self};
use crate::view::View;

/// The manipulator: hover highlighting, starting and running a drag, and
/// cancelling it with Escape.
///
/// Returns whether the pointer is the manipulator's this frame, so a click that
/// grabbed a handle does not also re-select whatever is behind it.
pub(crate) fn manipulate(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) -> bool {
    // Whether a move drag should snap to geometry this frame (issue 68), read
    // from the setting and, for the hold mode, the live key state. Read here on
    // every frame so a change of mode -- or the key going down mid-drag -- takes
    // effect at once.
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
    // A split counts as a group here: it has no dimensions of its own either,
    // and its pieces are meshes, which have none at all.
    let is_group = app.scene.node(id).is_group() || app.scene.node(id).is_split();
    let cursor = ui.input(|i| i.pointer.hover_pos());
    let dragging = app.drag.is_some();

    // The hover hit-test only matters while nothing is being dragged; during a
    // drag the grabbed handle is the one that counts.
    if !dragging {
        app.hover_handle = cursor.and_then(|cursor| gizmo.hit_test(view, cursor, is_group));
    }

    // **What was under the pointer when the button went down**, remembered here
    // and used to start the drag.
    //
    // A drag does not start until the pointer has moved past the toolkit's
    // threshold, and by then the pointer has left the handle it pressed: a
    // handle is grabbable within 9 px and the threshold is most of that. Asking
    // where the pointer is *now* therefore found no handle about half the time,
    // the press was reported as a plain click instead, and the object had to be
    // grabbed again -- sometimes several times over.
    let (pressed, press_origin) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.press_origin()));
    if pressed {
        app.grabbed = press_origin.and_then(|at| gizmo.hit_test(view, at, is_group));
    }

    // Everything the pointer has to say this frame, read off the response in one
    // place so the decision below needs nothing from egui.
    let pointer = gizmo::PointerState {
        escape: ui.input(|i| i.key_pressed(egui::Key::Escape)),
        released: response.drag_stopped() || ui.input(|i| i.pointer.any_released()),
        started: response.drag_started_by(egui::PointerButton::Primary),
        on_handle: app.grabbed.is_some(),
        have_cursor: cursor.is_some(),
    };
    let phase = gizmo::drag_phase(dragging, pointer);
    // The drag is measured from where the button went down, not from where the
    // pointer had already slipped to by the time the toolkit called it a drag.
    let from = if phase == gizmo::DragPhase::Begin { press_origin.or(cursor) } else { cursor };
    app.manipulate_step(&gizmo, view, id, phase, app.grabbed, from, mods_from(ui));

    let owned = app.drag.is_some() || app.grabbed.is_some();
    if pointer.released {
        app.grabbed = None;
    }
    owned
}

//! Orbiting, panning and zooming.

use super::*;
use crate::app::{App, Status};
use crate::view::View;
use simple3d_core::keymap::{Command, NavMap};
use simple3d_core::scene::Camera;

/// Apply a wheel scroll to the camera's distance, honouring the invert-zoom
/// binding.
///
/// `anchor` is the panel the wheel turned over and where the pointer was in it.
/// Given one, the zoom is about the pointer: whatever is under it stays under
/// it, so zooming in on a corner of the model walks the view towards that
/// corner instead of pulling the frame centre in and leaving the corner off the
/// side. The other half of moving freely about the grid (issue 72), pan being
/// the first. Pass `None` to zoom about the frame centre.
///
/// The projection is parallel, which is what makes this a two-line move rather
/// than a raycast: every world point along the pointer's ray sits at the same
/// offset from the target across the screen, so there is no depth to resolve
/// and no reference plane to choose. Zooming with the pointer over empty sky
/// works the same as zooming with it over the model.
pub fn apply_zoom(camera: &mut Camera, nav: &NavMap, scroll: f32, anchor: Option<(egui::Rect, egui::Pos2)>) {
    if scroll.abs() <= 0.01 {
        return;
    }
    let before = *camera;
    let direction = if nav.invert_zoom { -1.0 } else { 1.0 };
    let factor = (-scroll as f64 * direction * 0.0015).exp();
    camera.distance = (camera.distance * factor).clamp(0.05, 5.0e6);

    let Some((rect, cursor)) = anchor else {
        return;
    };
    // A panel with no area has no centre to measure the pointer from, and the
    // arithmetic below would hand the camera a target of NaN it could never be
    // steered back from.
    if !(rect.width() > 0.0 && rect.height() > 0.0) {
        return;
    }
    // The *realised* factor, not the asked-for one: at either end of the
    // distance range the clamp holds the zoom still, and the view has to hold
    // still with it rather than sliding sideways under a scroll that did
    // nothing.
    let factor = camera.distance / before.distance;
    let view = View::new(before, rect);
    let (right, up) = view.basis();
    let scale = view.mm_per_pixel_at(before.target);
    // Where the pointer is, as an offset from the target across the screen, in
    // millimetres. Screen y grows downward.
    let across = (cursor.x - view.centre.x) as f64 * scale;
    let upward = (view.centre.y - cursor.y) as f64 * scale;
    // That offset shrinks with the zoom, so the target moves the rest of the
    // way in to leave the point on screen where it was.
    camera.target = camera.target + (right * across + up * upward) * (1.0 - factor);
}

/// Orbit, pan and zoom, all on remappable bindings that take effect immediately
/// (spec section 8.2, acceptance criterion 29). The bindings are read from the
/// keymap on every frame, so a rebinding applies to the very next drag.
pub(crate) fn navigate(app: &mut App, ui: &mut egui::Ui, response: &egui::Response) {
    let nav = app.keymap.nav;
    let (ctrl, shift, alt) = ui.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));

    let held = [
        response.dragged_by(egui::PointerButton::Primary),
        response.dragged_by(egui::PointerButton::Middle),
        response.dragged_by(egui::PointerButton::Secondary),
    ];
    // A manipulator drag owns the pointer while it is running.
    let gesture = if app.drag.is_some() { None } else { nav_gesture(&nav, held, ctrl, shift, alt) };

    // A locked view centre pins the point the camera turns about (`AppSettings::
    // lock_view_centre`). Orbit and zoom still work -- they are what the pin is
    // for -- but the two gestures that carry the point itself leave it alone: a
    // pan, and the zoom's walk towards the pointer, which falls back to zooming
    // about the frame centre.
    let locked = app.settings.lock_view_centre;
    if let Some(gesture) = gesture {
        if locked && gesture == Gesture::Pan {
            // Said out loud, or a drag that does nothing reads as a drag that
            // does not work.
            app.status = Status::Info("The view centre is locked, so the pan moved nothing".into());
        } else {
            let view = app.current_view();
            apply_gesture(&mut app.scene.camera, gesture, response.drag_delta(), &view);
        }
    }

    if response.hovered() {
        let (scroll, at) = ui.input(|i| (i.smooth_scroll_delta.y, i.pointer.hover_pos()));
        let rect = app.viewport_rect;
        // Zooming about the pointer walks the view centre towards it, and the
        // centre is the one thing a zoom is not asked to move (issue 97): the
        // wheel changes how much is in frame, and where the frame sits is the
        // pan's business. So the walk is on a hold of its own -- one more key
        // than it took before, and the only way for a wheel to be both -- and
        // the plain wheel zooms about the frame centre. A locked centre stays
        // locked whatever is held.
        let towards_pointer =
            app.holding(Command::ZoomToPointer, |k| ui.input(|i| i.key_down(k)), ui.input(|i| i.modifiers));
        let anchor = if locked || !towards_pointer { None } else { at.map(|at| (rect, at)) };
        apply_zoom(&mut app.scene.camera, &nav, scroll, anchor);
    }
}

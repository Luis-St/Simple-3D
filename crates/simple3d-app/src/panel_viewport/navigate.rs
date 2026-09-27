//! Orbiting, panning and zooming.

use super::*;
use crate::app::{App, Status};
use crate::view::View;
use simple3d_core::keymap::{Command, NavMap};
use simple3d_core::scene::Camera;

/// Apply a wheel scroll to the camera distance, honouring invert-zoom. With an `anchor` (panel and
/// pointer) it zooms about the pointer, keeping what is under it in place (issue 72); `None` zooms
/// about the frame centre. Parallel projection makes this a simple offset, even over empty sky.
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
    // A zero-area panel would give the camera a NaN target it could never recover from.
    if !(rect.width() > 0.0 && rect.height() > 0.0) {
        return;
    }
    // The realised factor, so the view holds still when the distance clamp does.
    let factor = camera.distance / before.distance;
    let view = View::new(before, rect);
    let (right, up) = view.basis();
    let scale = view.mm_per_pixel_at(before.target);
    // The pointer's offset from the target across the screen, in millimetres; screen y grows downward.
    let across = (cursor.x - view.centre.x) as f64 * scale;
    let upward = (view.centre.y - cursor.y) as f64 * scale;
    // The offset shrinks with the zoom, so the target moves the rest of the way to keep the point put.
    camera.target = camera.target + (right * across + up * upward) * (1.0 - factor);
}

/// Orbit, pan and zoom on remappable bindings, read every frame so rebinding applies at once
/// (spec section 8.2, acceptance criterion 29).
pub(crate) fn navigate(app: &mut App, ui: &mut egui::Ui, response: &egui::Response) {
    let nav = app.keymap.nav;
    let (ctrl, shift, alt) = ui.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));

    let held = [
        response.dragged_by(egui::PointerButton::Primary),
        response.dragged_by(egui::PointerButton::Middle),
        response.dragged_by(egui::PointerButton::Secondary),
    ];
    // A manipulator drag owns the pointer while it runs.
    let gesture = if app.drag.is_some() { None } else { nav_gesture(&nav, held, ctrl, shift, alt) };

    // A locked view centre pins the orbit point: orbit and zoom still work, but pan and the
    // zoom-to-pointer walk leave it alone.
    let locked = app.settings.lock_view_centre;
    if let Some(gesture) = gesture {
        if locked && gesture == Gesture::Pan {
            // Said out loud, or the drag would seem broken.
            app.status = Status::Info("The view centre is locked, so the pan moved nothing".into());
        } else {
            let view = app.current_view();
            apply_gesture(&mut app.scene.camera, gesture, response.drag_delta(), &view);
        }
    }

    if response.hovered() {
        let (scroll, at) = ui.input(|i| (i.smooth_scroll_delta.y, i.pointer.hover_pos()));
        let rect = app.viewport_rect;
        // Zooming toward the pointer moves the view centre, which a plain zoom should not (issue 97),
        // so it needs its own hold; a locked centre stays locked regardless.
        let towards_pointer =
            app.holding(Command::ZoomToPointer, |k| ui.input(|i| i.key_down(k)), ui.input(|i| i.modifiers));
        let anchor = if locked || !towards_pointer { None } else { at.map(|at| (rect, at)) };
        apply_zoom(&mut app.scene.camera, &nav, scroll, anchor);
    }
}

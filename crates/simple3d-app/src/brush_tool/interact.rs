//! The brush's pointer: the circle follows the surface, a press and drag lays dabs along the path,
//! a press held still keeps dabbing, a click lays one, and the wheel with Ctrl sizes it.

use super::*;
use crate::app::Status;
use crate::view::View;
use simple3d_core::scene::NodeId;
use simple3d_geom::brush::Mode;

/// How finely the pointer's path between two frames is followed, in screen points: each sample
/// is cast at the surface, and a dab laid wherever the path has gone a spacing on.
const SAMPLE: f32 = 2.0;

/// Seconds between dabs while the pointer is held still on the surface: ten a second, so holding
/// raises at a steady pace (at the default size and strength, about 1.5 mm a second at the centre).
const HELD: f32 = 0.1;

/// The longest frame counted towards [`HELD`], so a stalled frame does not land a burst of dabs.
const MAX_DT: f32 = 0.1;

/// The wheel's effect on the size or strength per point scrolled.
const WHEEL: f64 = 0.002;

/// Whether the wheel sizes the brush rather than zooming: in the brush, with Ctrl held.
pub(crate) fn takes_wheel(app: &App, modifiers: egui::Modifiers) -> bool {
    app.mode == crate::gizmo::Mode::Brush && modifiers.command
}

/// The tool's pointer for one frame. Returns whether it took the pointer, so a press does not also
/// select what is under it.
pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) -> bool {
    let (cursor, modifiers) = ui.input(|i| (i.pointer.hover_pos(), i.modifiers));
    if takes_wheel(app, modifiers) && response.hovered() {
        // egui hands Ctrl and the wheel over as a zoom factor rather than a scroll; a scroll that does
        // come through counts too, sideways as well, since Shift turns a wheel's turn sideways.
        let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
        let factor = match (zoom - 1.0).abs() > 1e-6 {
            true => f64::from(zoom),
            false => (f64::from(scroll.x + scroll.y) * WHEEL).exp(),
        };
        if factor != 1.0 {
            match modifiers.shift {
                true => app.resize_brush(1.0, factor),
                false => app.resize_brush(factor, 1.0),
            }
        }
    }
    if app.brush.stroke.is_some() {
        if let Some(cursor) = cursor {
            let dt = ui.input(|i| i.stable_dt).min(MAX_DT);
            follow(app, view, cursor, modifiers, dt);
        }
        // Held still, the stroke keeps going without input events to wake the frame.
        ui.ctx().request_repaint();
        if response.drag_stopped() || ui.input(|i| i.pointer.any_released()) {
            app.brush.stroke = None;
            app.end_brush_stroke();
        }
        let stroked = app.sculpt.as_ref().map(|sculpt| sculpt.target);
        hover(app, cursor.and_then(|cursor| surface_under(app, view, cursor).filter(|hit| Some(hit.0) == stroked)));
        return true;
    }
    hover(app, cursor.filter(|_| response.hovered()).and_then(|cursor| surface_under(app, view, cursor)));
    // The stroke starts on the press itself, not once egui calls it a drag, so a press held still
    // brushes right away; a click is then a stroke that ends a frame later.
    let pressed = response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed());
    let Some(id) = app.brush.over.filter(|_| pressed) else {
        if pressed {
            app.status = Status::Info(hint(app).into());
        }
        return false;
    };
    // The body pressed on is the one brushed, selected so the panels show what the stroke edits.
    if target(app) != Some(id) {
        app.select_only(id);
    }
    let press = ui.input(|i| i.pointer.press_origin()).or(cursor).expect("a press has a position");
    if !app.begin_brush_stroke() {
        return false;
    }
    let (origin, dir) = view.ray(press);
    let Some(first) = app.brush_dab(origin, dir, mode(app, modifiers)) else {
        app.cancel_stroke();
        return false;
    };
    app.brush.stroke = Some(Dragging { pointer: press, last: first, held: 0.0 });
    true
}

/// The mode a dab is laid in: the brush's, inverted with Ctrl, or smoothing with Shift held.
fn mode(app: &App, modifiers: egui::Modifiers) -> Mode {
    let chosen = app.settings.last_brush.mode;
    match (modifiers.shift, modifiers.command) {
        (true, _) => Mode::Smooth,
        (false, true) => chosen.inverted(),
        (false, false) => chosen,
    }
}

/// Follow the pointer from last frame's position to `cursor`, laying a dab wherever the path has
/// gone a spacing on over the surface, and one every [`HELD`] seconds while it stays put.
fn follow(app: &mut App, view: &View, cursor: egui::Pos2, modifiers: egui::Modifiers, dt: f32) {
    let Some(stroke) = app.brush.stroke.as_ref() else { return };
    let (from, mut last, mut held) = (stroke.pointer, stroke.last, stroke.held);
    let spacing = app.settings.last_brush.spacing();
    let steps = ((cursor - from).length() / SAMPLE).ceil() as usize;
    let mode = mode(app, modifiers);
    let mut laid = false;
    for step in 1..=steps {
        let at = from + (cursor - from) * (step as f32 / steps as f32);
        let (origin, dir) = view.ray(at);
        let Some(sculpt) = app.sculpt.as_ref() else { return };
        let Some((t, _)) = sculpt.ray(origin, dir) else { continue };
        if (origin + dir * t - last).length() < spacing {
            continue;
        }
        if let Some(landed) = app.brush_dab(origin, dir, mode) {
            (last, laid) = (landed, true);
        }
    }
    held = if laid { 0.0 } else { held + dt };
    while held >= HELD {
        held -= HELD;
        let (origin, dir) = view.ray(cursor);
        if let Some(landed) = app.brush_dab(origin, dir, mode) {
            last = landed;
        }
    }
    app.brush.stroke = Some(Dragging { pointer: cursor, last, held });
}

/// Remember what is under the pointer: the body, and the circle's place on it.
fn hover(app: &mut App, hit: Option<(NodeId, Vec3, Vec3)>) {
    app.brush.over = hit.map(|(id, _, _)| id);
    app.brush.hover = hit.map(|(_, at, normal)| (at, normal));
}

/// The mesh body under `cursor`, with the point and normal of its surface there: the working copy
/// while it is drawn, else the nearest shown body, when that is a mesh body.
fn surface_under(app: &App, view: &View, cursor: egui::Pos2) -> Option<(NodeId, Vec3, Vec3)> {
    let (origin, dir) = view.ray(cursor);
    if let Some(sculpt) = app.sculpt_shown() {
        if let Some((t, triangle)) = sculpt.ray(origin, dir) {
            let mesh = &sculpt.renderable.mesh;
            return Some((sculpt.target, origin + dir * t, facing(mesh.triangle_normal(mesh.indices[triangle]), dir)));
        }
    }
    let id = crate::pick::pick(&app.scene, &app.evaluated, origin, dir)?;
    app.scene.get(id).filter(|node| node.mesh().is_some())?;
    let mesh = app.evaluated.node_meshes.get(&id)?;
    let (t, triangle) = crate::pick::ray_mesh_triangle(mesh, origin, dir)?;
    Some((id, origin + dir * t, facing(mesh.triangle_normal(mesh.indices[triangle]), dir)))
}

/// `normal` turned towards the eye, so the circle sits on the side looked at.
fn facing(normal: Vec3, dir: Vec3) -> Vec3 {
    if normal.dot(dir) > 0.0 {
        normal * -1.0
    } else {
        normal
    }
}

//! The 3D cursor: where it is put and what it snaps to.

use crate::app::{App, Status};
use crate::theme::token;
use crate::view::View;
use simple3d_geom::Vec3;

/// The resize cursor matching a screen direction, so a handle shows its travel before it is grabbed.
/// Folded into a half-circle (a line has no direction) in 45-degree sectors; screen y grows down.
pub fn slide_cursor(along: egui::Vec2) -> egui::CursorIcon {
    if along.length_sq() < 1e-6 {
        return egui::CursorIcon::ResizeHorizontal;
    }
    let bearing = along.y.atan2(along.x).to_degrees().rem_euclid(180.0);
    match bearing {
        b if !(22.5..157.5).contains(&b) => egui::CursorIcon::ResizeHorizontal,
        b if b < 67.5 => egui::CursorIcon::ResizeNwSe,
        b if b < 112.5 => egui::CursorIcon::ResizeVertical,
        _ => egui::CursorIcon::ResizeNeSw,
    }
}

/// The 3D cursor as small crosshairs, reading as a position rather than part of the model.
pub(crate) fn draw_cursor(app: &App, painter: &egui::Painter, view: &View) {
    let Some(at) = app.cursor else { return };
    let Some((screen, _)) = view.project(at) else { return };
    let r = 9.0;
    for (dx, dy) in [(1.0, 0.0), (0.0, 1.0)] {
        painter.line_segment(
            [screen - egui::vec2(dx, dy) * r, screen + egui::vec2(dx, dy) * r],
            egui::Stroke::new(1.0_f32, token::MEASURE),
        );
    }
    painter.circle_stroke(screen, r * 0.55, egui::Stroke::new(1.0_f32, token::MEASURE));
}

/// Shift+right-click puts the cursor on what is under the pointer, or the ground; on empty space
/// away from the ground, back at the origin.
pub(crate) fn place_cursor(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    let shift = ui.input(|i| i.modifiers.shift);
    let pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary));
    if !shift || !pressed {
        return;
    }
    let Some(pointer) = response.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.hover_pos())) else {
        return;
    };
    // Prefer the surface under the pointer: placing against another shape is the point.
    let hit = app.surface_under(view, pointer);
    let at = hit.or_else(|| view.ray_plane_ahead(pointer, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)));
    match at {
        Some(at) => {
            let snapped = snap_point(at, app.move_snap());
            app.cursor = Some(snapped);
            app.status = Status::Info(format!("3D cursor at {}", crate::ui::describe_point(snapped, app.unit())));
        }
        None => {
            app.cursor = None;
            app.status = Status::Info("3D cursor back at the origin".into());
        }
    }
}

/// Snap to the move grid, so cursor-placed shapes land on the same numbers as moved ones.
pub fn snap_point(p: Vec3, step: f64) -> Vec3 {
    if step <= 0.0 {
        return p;
    }
    Vec3::new((p.x / step).round() * step, (p.y / step).round() * step, (p.z / step).round() * step)
}

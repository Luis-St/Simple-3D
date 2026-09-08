//! The 3D cursor: where it is put and what it snaps to.

use crate::app::{App, Status};
use crate::pick;
use crate::theme::token;
use crate::view::View;
use simple3d_geom::Vec3;

/// The resize cursor that matches a direction on screen, so a handle says which
/// way it will move before it is grabbed rather than always claiming to slide
/// left and right.
///
/// The four cursors cover the half-circle in 45-degree sectors, and a line has
/// no sense of direction -- pushing and pulling along it are the same
/// gesture -- so the bearing is folded into that half-circle first. Screen y
/// grows downward, which is why "right and down" is the north-west/south-east
/// diagonal rather than the other one.
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

/// Where a shape would land: the 3D cursor, drawn as a small set of crosshairs
/// so it reads as a position rather than as a piece of the model.
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

/// Shift and the orbit button's opposite -- the right button -- puts the cursor
/// on whatever is under the pointer, or on the ground plane when that is
/// nothing. Shift+right-click again on empty space away from the ground puts it
/// back at the origin.
pub(crate) fn place_cursor(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    let shift = ui.input(|i| i.modifiers.shift);
    let pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary));
    if !shift || !pressed {
        return;
    }
    let Some(pointer) = response.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.hover_pos())) else {
        return;
    };
    // Prefer the surface actually under the pointer: placing a shape against
    // another shape is the reason to move the cursor at all.
    let (origin, dir) = view.ray(pointer);
    let hit = pick::ray_mesh(&app.evaluated.mesh, origin, dir).map(|t| origin + dir * t);
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

/// The cursor snaps to the same grid a move does, so a shape placed with it
/// lands on the same numbers a shape moved with the handles does.
pub fn snap_point(p: Vec3, step: f64) -> Vec3 {
    if step <= 0.0 {
        return p;
    }
    Vec3::new((p.x / step).round() * step, (p.y / step).round() * step, (p.z / step).round() * step)
}

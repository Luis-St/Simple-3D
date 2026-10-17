//! The 3D cursor: where it is put and what it snaps to.

use crate::app::{App, Status};
use crate::theme::token;
use crate::view::View;

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

/// Shift+right-click puts the cursor where a position pick lands (issue 126): what it snaps to, else
/// the surface under the pointer or the ground, on the move grid; on empty space away from the ground,
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
    match app.pick_snapping(view, pointer, &[], cursor_snaps(app, ui)) {
        Some((point, _)) => {
            // A caught feature is exact; anywhere else lands on the same numbers as moved shapes.
            let snapped =
                if point.kind.is_some() { point.at } else { crate::snap::on_grid(point.at, None, app.move_snap()) };
            app.cursor = Some(snapped);
            app.status = Status::Info(format!("3D cursor at {}", crate::ui::describe_point(snapped, app.unit())));
        }
        None => {
            app.cursor = None;
            app.status = Status::Info("3D cursor back at the origin".into());
        }
    }
}

/// What a Shift+right-click would snap the cursor to, while Shift is held and snapping is asked for
/// (issue 126), so the snap is seen before the click.
pub(crate) fn draw_cursor_snap(app: &App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    let free = app.placing.is_none() && !app.measure.active && app.drag.is_none();
    if !free || !ui.input(|i| i.modifiers.shift) || !cursor_snaps(app, ui) {
        return;
    }
    let Some(cursor) = ui.input(|i| i.pointer.hover_pos()).filter(|at| painter.clip_rect().contains(*at)) else {
        return;
    };
    if let Some((_, Some(mark))) = app.snap_catch(view, cursor, &[]) {
        super::draw_snap_mark(painter, view, &mark, token::MEASURE);
    }
}

/// Whether a cursor placement snaps. Shift is the gesture's own, so the snap key counts with it held
/// on top: Shift+Ctrl snaps under the default Ctrl.
fn cursor_snaps(app: &App, ui: &egui::Ui) -> bool {
    let mods = egui::Modifiers { shift: false, ..ui.input(|i| i.modifiers) };
    app.snap_requested || app.geometry_snap_wanted(|k| ui.input(|i| i.key_down(k)), mods)
}

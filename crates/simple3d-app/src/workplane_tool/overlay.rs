//! What is drawn over the picture for workplanes: the face a click would put one on, and the
//! origin and directions of the one set up or active. The grid itself is the renderer's.

use super::*;
use crate::app::App;
use crate::theme::token;
use crate::view::View;

/// How long the drawn X and Y arms are, in pixels.
const ARM: f64 = 46.0;

/// The hovered face faintly while the tool is out, then the origin and X and Y arms of the workplane
/// being set up, or of the active one. Drawn over the picture, so they stay readable behind a body.
pub(crate) fn draw(app: &mut App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    let screen = |p: Vec3| view.project(p).map(|(at, _)| at);
    if app.workplane_tool.is_some() {
        let hover = ui.input(|i| i.pointer.hover_pos()).filter(|at| painter.clip_rect().contains(*at));
        if let Some(face) = hover.and_then(|cursor| crate::cut_tool::face_under(app, view, cursor)) {
            let (u, v) = face.axes();
            let size = 12.0 / view.pixels_per_mm().max(1e-9);
            let corner = |s: f64, t: f64| screen(face.point + u * (s * size) + v * (t * size));
            let corners = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
            for i in 0..4 {
                if let (Some(a), Some(b)) = (corners[i], corners[(i + 1) % 4]) {
                    painter.line_segment([a, b], egui::Stroke::new(1.5_f32, token::ACCENT.gamma_multiply(0.6)));
                }
            }
        }
    }
    let placing = app.workplane_tool.as_ref().and_then(WorkplaneTool::preview);
    let Some(plane) = placing.as_ref().or(app.workplane()) else { return };
    let (origin, [x, y, _]) = (plane.origin(), plane.axes());
    let Some(at) = screen(origin) else { return };
    let arm = ARM / view.pixels_per_mm().max(1e-9);
    // Strong while being set up, quiet once it is only the plane being worked on.
    let strength = if placing.is_some() { 1.0 } else { 0.55 };
    for (axis, direction) in [(0, x), (1, y)] {
        if let Some(end) = screen(origin + direction * arm) {
            let colour = crate::gizmo::axis_colour(axis).gamma_multiply(strength);
            painter.line_segment([at, end], egui::Stroke::new(2.0_f32, colour));
        }
    }
    let colour = token::ACCENT.gamma_multiply(strength);
    painter.rect_filled(egui::Rect::from_center_size(at, egui::Vec2::splat(5.0)), 1.0, colour);
}

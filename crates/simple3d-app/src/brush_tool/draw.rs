//! The brush circle, lying on the surface under the pointer.

use super::*;
use crate::theme::token;
use crate::view::View;

/// Points round the circle; enough that it reads round at any size.
const SEGMENTS: usize = 64;

/// The circle's line width, in points.
const LINE: f32 = 1.5;

/// The circle at the hovered surface point, in the plane facing along its normal, with a short tick
/// along the normal at its centre.
pub(crate) fn draw(app: &App, painter: &egui::Painter, view: &View) {
    if app.mode != crate::gizmo::Mode::Brush {
        return;
    }
    let Some((centre, normal)) = app.brush.hover else { return };
    let radius = app.settings.last_brush.radius;
    let across = if normal.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let u = normal.cross(across).normalized();
    let v = normal.cross(u);
    let ring: Vec<egui::Pos2> = (0..SEGMENTS)
        .filter_map(|k| {
            let angle = std::f64::consts::TAU * k as f64 / SEGMENTS as f64;
            view.project(centre + (u * angle.cos() + v * angle.sin()) * radius).map(|(at, _)| at)
        })
        .collect();
    if ring.len() == SEGMENTS {
        painter.add(egui::Shape::closed_line(ring, egui::Stroke::new(LINE, token::ACCENT)));
    }
    let tick = [centre, centre + normal * (radius * 0.3)].map(|p| view.project(p).map(|(at, _)| at));
    if let [Some(from), Some(to)] = tick {
        painter.line_segment([from, to], egui::Stroke::new(LINE, token::ACCENT));
    }
}

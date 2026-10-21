//! What the tool draws over the picture: the plane, where the cut runs through each object, and the
//! face a click would pick.

use super::*;
use crate::app::App;
use crate::theme::token;
use crate::view::View;

/// The plane as a frame, the area's or one round everything it cuts, each object's cross-section
/// solid on it, and the face under the pointer faintly. Drawn over the picture, so a cut behind a
/// body stays readable.
pub(crate) fn draw(app: &mut App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    let Some(tool) = app.cut_tool.as_ref() else { return };
    let colour = token::ACCENT;
    let screen = |p: Vec3| view.project(p).map(|(at, _)| at);
    let line = |a: Vec3, b: Vec3, width: f32, colour: egui::Color32| {
        if let (Some(a), Some(b)) = (screen(a), screen(b)) {
            painter.line_segment([a, b], egui::Stroke::new(width, colour));
        }
    };
    let frame = |face: &CutPlane, half: [f64; 2], width: f32, colour: egui::Color32| {
        let (u, v) = face.axes();
        let corner = |s: f64, t: f64| face.point + u * (s * half[0]) + v * (t * half[1]);
        let corners = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
        for i in 0..4 {
            line(corners[i], corners[(i + 1) % 4], width, colour);
        }
    };

    // What a click would pick, only over the viewport itself.
    if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()).filter(|at| painter.clip_rect().contains(*at)) {
        if let Some(hover) = face_under(app, view, cursor) {
            let size = 12.0 / view.pixels_per_mm().max(1e-9);
            frame(&hover, [size, size], 1.5, colour.gamma_multiply(0.6));
        }
    }

    let Some(face) = tool.face else { return };
    let (limited, size) = (tool.limited, tool.size);
    let planned = app.cut_planned();
    let half = if limited { [size[0] / 2.0, size[1] / 2.0] } else { reach(&face, &planned) };
    frame(&face, half, 1.0, colour.gamma_multiply(0.45));
    if let Some(at) = screen(face.point) {
        painter.rect_filled(egui::Rect::from_center_size(at, egui::Vec2::splat(5.0)), 1.0, colour);
    }
    for object in planned.iter() {
        for &[a, b] in &object.outline {
            line(a, b, 2.0, colour);
        }
    }
}

/// How far the endless plane's frame runs along each of its axes from the face's middle: over every
/// object it cuts, with a margin, or a little round the face when it cuts nothing.
fn reach(face: &CutPlane, planned: &[Planned]) -> [f64; 2] {
    let (u, v) = face.axes();
    let mut most: [f64; 2] = [0.0, 0.0];
    for object in planned {
        let (lo, hi) = object.bounds;
        for corner in 0..8 {
            let offset = simple3d_geom::aabb::box_corner(lo, hi, corner) - face.point;
            most = [most[0].max(offset.dot(u).abs()), most[1].max(offset.dot(v).abs())];
        }
    }
    let margin = 0.1 * most[0].max(most[1]).max(10.0);
    [most[0] + margin, most[1] + margin]
}

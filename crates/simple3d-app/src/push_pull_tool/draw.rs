//! What the tool draws over the picture: the face it would take, and how far it is dragged.

use super::*;
use crate::app::App;
use crate::theme::token;
use crate::view::View;
use simple3d_geom::Vec3;

/// The hovered face tinted, or the dragged distance at the pointer.
pub(crate) fn draw(app: &App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    if app.mode != crate::gizmo::Mode::PushPull {
        return;
    }
    if let Some(drag) = &app.push_pull.drag {
        // Yellow while only hovered or held, green once it sweeps a solid. The swept solid is in the
        // picture itself (`prism.rs`), where walls in front hide it; until
        // there is one, the face held is tinted, being in front of everything along the pointer.
        if drag.prism.is_none() {
            tint(app, painter, view, &drag.hit, token::ACCENT.gamma_multiply(0.25));
        }
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            let unit = app.unit();
            let text = format!(
                "{}{} {}",
                if drag.distance > 0.0 { "+" } else { "" },
                simple3d_core::unit::format_length(drag.distance, unit),
                unit.suffix()
            );
            readout(painter, cursor + egui::vec2(14.0, -18.0), &text);
        }
        return;
    }
    let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) else { return };
    if !painter.clip_rect().contains(cursor) {
        return;
    }
    if let Some((Ok(hit), _)) = app.face_under(view, cursor) {
        tint(app, painter, view, &hit, token::ACCENT.gamma_multiply(0.18));
        for points in &hit.face.loops {
            outline(painter, view, points, egui::Stroke::new(1.5_f32, token::ACCENT));
        }
    }
}

/// The face's own triangles filled, so the whole region a drag would take is visible.
fn tint(app: &App, painter: &egui::Painter, view: &View, hit: &FaceHit, colour: egui::Color32) {
    let mesh = &app.evaluated.mesh;
    let mut shape = egui::epaint::Mesh::default();
    for &t in &hit.face.triangles {
        let Some(tri) = mesh.indices.get(t) else { continue };
        let corners: Vec<egui::Pos2> =
            tri.iter().filter_map(|&v| view.project(mesh.positions[v as usize]).map(|(p, _)| p)).collect();
        if corners.len() != 3 {
            continue;
        }
        let base = shape.vertices.len() as u32;
        for p in corners {
            shape.colored_vertex(p, colour);
        }
        shape.add_triangle(base, base + 1, base + 2);
    }
    painter.add(egui::Shape::mesh(shape));
}

fn outline(painter: &egui::Painter, view: &View, points: &[Vec3], stroke: egui::Stroke) {
    let screen: Vec<egui::Pos2> = points.iter().filter_map(|&p| view.project(p).map(|(s, _)| s)).collect();
    if screen.len() == points.len() && screen.len() > 2 {
        painter.add(egui::Shape::closed_line(screen, stroke));
    }
}

/// A measurement at the pointer, like a handle drag's.
pub(crate) fn readout(painter: &egui::Painter, at: egui::Pos2, text: &str) {
    let galley = painter.layout_no_wrap(text.to_string(), egui::FontId::monospace(13.0), token::MEASURE);
    let background = egui::Rect::from_min_size(at, galley.size()).expand(5.0);
    painter.rect_filled(background, 3.0, token::SURFACE_1.gamma_multiply(0.92));
    painter.rect_stroke(
        background,
        3.0,
        egui::Stroke::new(1.0_f32, token::MEASURE.gamma_multiply(0.5)),
        egui::StrokeKind::Inside,
    );
    painter.galley(at, galley, token::MEASURE);
}

//! The picks drawn over the picture; the treatment itself is in the picture (`ghost.rs`).

use super::pick::Pick;
use super::*;
use crate::theme::token;
use crate::view::View;
use simple3d_geom::Vec3;

/// Picked edges thick, picked corners as squares, what a click would pick now thin, and the size at
/// the pointer while an edge is dragged.
pub(crate) fn draw(app: &App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    let Some(tool) = app.round_tool.as_ref() else { return };
    let screen = |p: Vec3| view.project(p).map(|(s, _)| s);
    let line = |a: Vec3, b: Vec3, stroke: egui::Stroke| {
        if let (Some(a), Some(b)) = (screen(a), screen(b)) {
            painter.line_segment([a, b], stroke);
        }
    };
    // Picks too small for the size are drawn red: they are left as they are.
    let treatment = tool.treatment();
    for edge in &tool.edges {
        let colour = if tool.fits(edge) { token::ACCENT } else { token::DANGER };
        line(edge.a, edge.b, egui::Stroke::new(3.0_f32, colour));
    }
    for corner in &tool.corners {
        let fits = simple3d_geom::rounding::corner_fits(corner, treatment);
        if let Some(at) = screen(corner.at) {
            let colour = if fits { token::ACCENT } else { token::DANGER };
            painter.rect_filled(egui::Rect::from_center_size(at, egui::Vec2::splat(9.0)), 1.0, colour);
        }
    }
    // Inside corners as diamonds, to tell them from the blended outside corners.
    for &joint in &tool.joints {
        if let Some(at) = screen(joint) {
            painter.add(egui::Shape::convex_polygon(diamond(at, 6.0), token::ACCENT, egui::Stroke::NONE));
        }
    }

    let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) else { return };
    if tool.drag.is_some() {
        let unit = app.unit();
        let size = simple3d_core::unit::format_length(tool.size(), unit);
        let name = match tool.kind {
            Kind::Round => "Radius",
            Kind::Chamfer => "Distance",
        };
        let mut text = format!("{name} {size} {}", unit.suffix());
        match tool.too_small() {
            0 => {}
            n => text.push_str(&format!(" ({n} too small)")),
        }
        crate::push_pull_tool::draw::readout(painter, cursor + egui::vec2(14.0, -18.0), &text);
        return;
    }
    if !painter.clip_rect().contains(cursor) {
        return;
    }
    let hover = egui::Stroke::new(2.0_f32, token::ACCENT.gamma_multiply(0.6));
    match app.round_pick_at(view, cursor) {
        Some(Pick::Edge(edge)) => line(edge.a, edge.b, hover),
        Some(Pick::Joint(joint)) => {
            if let Some(at) = screen(joint) {
                painter.add(egui::Shape::closed_line(diamond(at, 7.5), hover));
            }
        }
        Some(Pick::Corner(corner)) => {
            if let Some(at) = screen(corner.at) {
                painter.rect_stroke(
                    egui::Rect::from_center_size(at, egui::Vec2::splat(11.0)),
                    1.0,
                    hover,
                    egui::StrokeKind::Middle,
                );
            }
        }
        None => {}
    }
}

/// A square standing on a corner, `r` from its centre to each tip.
fn diamond(at: egui::Pos2, r: f32) -> Vec<egui::Pos2> {
    vec![at + egui::vec2(0.0, -r), at + egui::vec2(r, 0.0), at + egui::vec2(0.0, r), at + egui::vec2(-r, 0.0)]
}

//! What the tool draws: templates in the picture, and the path over it.

use super::*;
use crate::app::App;
use crate::theme::token;
use crate::view::View;
use simple3d_core::xform::Xform;

/// Every object that would move and every copy that would be made, with what carries it there, for
/// the renderer to draw translucent (issue 70). Empty while the settings come to nothing.
pub(crate) fn templates(app: &App) -> Vec<(NodeId, Xform)> {
    if app.arrange_tool.is_none() {
        return Vec::new();
    }
    app.arrange_plan()
        .unwrap_or_default()
        .into_iter()
        .filter(Placement::moves)
        .map(|placement| (placement.source, placement.world))
        .collect()
}

/// The path, where each object's centre lands on it, and what a click would add, over the picture.
/// Drawn over rather than in the picture, so a path behind a body stays readable.
pub(crate) fn draw(app: &App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    if app.arrange_tool.as_ref().is_some_and(|tool| tool.mode == Arrange::OntoFace) {
        draw_onto(app, ui, painter, view);
        return;
    }
    let Some(tool) = app.arrange_tool.as_ref().filter(|tool| tool.mode == Arrange::Path) else { return };
    let colour = token::ACCENT;
    let screen = |p: Vec3| view.project(p).map(|(at, _)| at);
    let line = |a: Vec3, b: Vec3, width: f32, colour: egui::Color32| {
        if let (Some(a), Some(b)) = (screen(a), screen(b)) {
            painter.line_segment([a, b], egui::Stroke::new(width, colour));
        }
    };
    let path = tool.path();
    for (a, b) in path.segments() {
        line(a, b, 2.0, colour);
    }
    for &point in &path.points {
        if let Some(at) = screen(point) {
            painter.rect_filled(egui::Rect::from_center_size(at, egui::Vec2::splat(5.0)), 1.0, colour);
        }
    }
    // Where the objects' centres will be, hollow so the path shows through.
    for spot in path.spread(tool.count.max(tool.targets.len())).into_iter().filter_map(|s| path.at(s)) {
        if let Some(at) = screen(spot.0) {
            painter.circle_stroke(at, 4.5, egui::Stroke::new(1.5_f32, token::TEXT_HI));
        }
    }

    // What the next click would add, only over the viewport itself.
    let Some(cursor) = ui.input(|i| i.pointer.hover_pos()).filter(|at| painter.clip_rect().contains(*at)) else {
        return;
    };
    match tool.source {
        PathSource::Edges => {
            for (a, b) in super::run_under(app, view, cursor).unwrap_or_default() {
                line(a, b, 3.0, colour.gamma_multiply(0.6));
            }
        }
        PathSource::Drawn => {
            let Some((hover, caught)) = app.pick_position(view, cursor, &[]) else { return };
            match &caught {
                Some(caught) => crate::panel_viewport::draw_snap_mark(painter, view, caught, colour),
                None => {
                    if let Some(at) = screen(hover.at) {
                        painter.circle_stroke(at, 4.0, egui::Stroke::new(1.5_f32, colour));
                    }
                }
            }
            // A rubber band from the last point, to aim the next.
            if let Some(&last) = tool.points.last() {
                line(last, hover.at, 1.0, colour.gamma_multiply(0.6));
            }
        }
    }
}

/// The picked face's landing point with its normal, and where a click would pick (issue 118).
fn draw_onto(app: &App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    let Some(tool) = app.arrange_tool.as_ref() else { return };
    let colour = token::ACCENT;
    let mark = |at: Vec3, normal: Vec3, colour: egui::Color32| {
        let Some((screen, _)) = view.project(at) else { return };
        painter.circle_stroke(screen, 5.0, egui::Stroke::new(1.5_f32, colour));
        let reach = normal * (24.0 * view.mm_per_pixel_at(at));
        if let Some((tip, _)) = view.project(at + reach) {
            painter.line_segment([screen, tip], egui::Stroke::new(1.5_f32, colour));
        }
    };
    if let Some(landing) = tool.landing {
        let subjects = app.arrange_subjects();
        if !subjects.is_empty() {
            let at = super::onto::landing_point(tool.onto, &landing, super::onto::middle(&subjects));
            mark(at, landing.normal, colour);
        }
    }
    let Some(cursor) = ui.input(|i| i.pointer.hover_pos()).filter(|at| painter.clip_rect().contains(*at)) else {
        return;
    };
    if let Some((at, _, normal)) = super::hit_under(app, view, cursor) {
        mark(at, normal, colour.gamma_multiply(0.6));
    }
}

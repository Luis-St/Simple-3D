//! The measure tool's span, drawn over the picture.

use crate::app::{App, Measurement};
use crate::theme::token;
use crate::view::View;
use simple3d_geom::Vec3;

/// The measure tool's marks: the placed points, the span between them once both
/// are down, its numbers, and -- while one end is placed -- a live line to the
/// feature under the pointer (issue 69). All in the measure colour, because it
/// reads distances and never touches the model.
pub(crate) fn draw_measure(app: &App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    let colour = token::MEASURE;
    let mark = |at: Vec3, snapped: bool| {
        if let Some((screen, _)) = view.project(at) {
            // A snapped point gets a hollow square, an on-surface one a small
            // cross, so a glance says whether it caught a feature.
            if snapped {
                painter.rect_stroke(
                    egui::Rect::from_center_size(screen, egui::Vec2::splat(9.0)),
                    1.0,
                    egui::Stroke::new(1.5_f32, colour),
                    egui::StrokeKind::Middle,
                );
            } else {
                let r = 5.0;
                painter.line_segment(
                    [screen - egui::vec2(r, r), screen + egui::vec2(r, r)],
                    egui::Stroke::new(1.5_f32, colour),
                );
                painter.line_segment(
                    [screen - egui::vec2(r, -r), screen + egui::vec2(r, -r)],
                    egui::Stroke::new(1.5_f32, colour),
                );
            }
        }
    };

    for point in &app.measure.points {
        mark(point.at, point.kind.is_some());
    }

    // Where the *next* click would land, marked and named before it is made.
    //
    // This used to be drawn only once one end was down, which is exactly
    // backwards: the first point is the one placed with nothing else on screen
    // to judge it against, and it was placed blind (issue 78). The hover point is
    // resolved by the same call a click makes, so what is shown is what would be
    // taken -- including a point part-way along an edge, or where an axis crosses
    // a body.
    if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
        // Only over the viewport itself: the pointer out over a dock is not
        // aiming at anything in the scene.
        if painter.clip_rect().contains(cursor) {
            if let Some(hover) = app.measure_point_at(view, cursor) {
                mark(hover.at, hover.kind.is_some());
                // Name the feature the pointer has caught, so a snap is legible
                // rather than a guess.
                if let (Some(kind), Some((screen, _))) = (hover.kind, view.project(hover.at)) {
                    painter.text(
                        screen + egui::vec2(11.0, -11.0),
                        egui::Align2::LEFT_BOTTOM,
                        kind.label(),
                        egui::FontId::proportional(10.0),
                        colour,
                    );
                }
                // A live line from the first point, so the second click can be
                // aimed.
                if app.measure.points.len() == 1 {
                    let ends = (view.project(app.measure.points[0].at), view.project(hover.at));
                    if let (Some((a, _)), Some((b, _))) = ends {
                        painter.line_segment([a, b], egui::Stroke::new(1.0_f32, colour.gamma_multiply(0.6)));
                    }
                }
            }
        }
    }

    let Some((a, b)) = app.measure.span() else { return };
    let (Some((sa, _)), Some((sb, _))) = (view.project(a.at), view.project(b.at)) else { return };
    painter.line_segment([sa, sb], egui::Stroke::new(2.0_f32, colour));

    // The numbers, in a small panel by the middle of the span, kept there until
    // the tool is dismissed.
    let m = Measurement::between(a.at, b.at);
    let unit = app.unit();
    let suffix = unit.suffix();
    let fmt = |v: f64| simple3d_core::unit::format_length(v, unit);
    let lines = [
        format!("Distance  {}{suffix}", fmt(m.distance)),
        format!("\u{0394}  {}, {}, {} {suffix}", fmt(m.delta.x), fmt(m.delta.y), fmt(m.delta.z)),
        format!(
            "Incline  {}\u{00B0}   Bearing  {}\u{00B0}",
            simple3d_core::unit::format_angle(m.inclination_deg),
            simple3d_core::unit::format_angle(m.bearing_deg)
        ),
    ];
    let at = ((sa.to_vec2() + sb.to_vec2()) / 2.0).to_pos2() + egui::vec2(10.0, 8.0);
    let mut galleys = Vec::new();
    let mut size = egui::Vec2::ZERO;
    for line in &lines {
        let galley = painter.layout_no_wrap(line.clone(), egui::FontId::monospace(12.0), colour);
        size.x = size.x.max(galley.size().x);
        size.y += galley.size().y;
        galleys.push(galley);
    }
    let background = egui::Rect::from_min_size(at, size).expand(6.0);
    painter.rect_filled(background, 3.0, token::SURFACE_1.gamma_multiply(0.94));
    painter.rect_stroke(
        background,
        3.0,
        egui::Stroke::new(1.0_f32, colour.gamma_multiply(0.5)),
        egui::StrokeKind::Inside,
    );
    let mut y = at.y;
    for galley in galleys {
        let h = galley.size().y;
        painter.galley(egui::pos2(at.x, y), galley, colour);
        y += h;
    }
}

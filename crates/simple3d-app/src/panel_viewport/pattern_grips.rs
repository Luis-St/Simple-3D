//! The handles a pattern is laid out by.

use super::*;
use crate::app::App;
use crate::theme::token;
use crate::view::View;
use simple3d_geom::Vec3;

/// The lay-out grips of the selected pattern, if it is one (issue 67).
///
/// Every kind offers its own: the spacing and the number of copies along each
/// straight run, the radius and the span of a ring, the radius, rise and length
/// of a helix, the radii of a spiral. A mirror offers none -- it has neither a
/// distance nor a count -- and neither does a pattern whose numbers put every
/// grip on its own origin, where the move manipulator already is.
pub(crate) fn pattern_grips(app: &App) -> Vec<crate::app::PatternGrip> {
    match app.primary() {
        Some(id) => app.pattern_grips(id),
        None => Vec::new(),
    }
}

/// Drag a pattern's grips to lay it out by eye. Returns whether the pointer is
/// theirs this frame.
///
/// Each grip is its own widget, keyed by its label rather than its position in
/// the list: dragging the "Copies" grip *adds* copies, which can add a "Spacing"
/// grip beside it, and an index would shift out from under the drag that caused
/// it.
pub(crate) fn pattern_grips_interact(app: &mut App, ui: &mut egui::Ui, view: &View) -> bool {
    let Some(id) = app.primary() else { return false };
    let mut owned = false;
    for grip in pattern_grips(app) {
        let Some((screen, _)) = view.project(grip.at) else { continue };
        let rect = egui::Rect::from_center_size(screen, egui::Vec2::splat(16.0));
        let response = ui
            .interact(rect, ui.id().with((id, "pattern-grip", grip.label)), egui::Sense::drag())
            .on_hover_text(grip.label);
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(match grip.turn {
                // A turn is not a push or a pull, and there is no cursor for
                // "round": the hand says the grip is held and the arc under it
                // says which way it goes.
                Some(_) => egui::CursorIcon::Grabbing,
                // Every other grip slides along a line, and the pointer says
                // which line: an arrow across the screen for a run that lies
                // across it, and up the screen for one that stands up.
                None => slide_cursor(screen_direction(view, grip.at, grip.dir)),
            });
        }
        if response.dragged() {
            if let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) {
                if let Some(value) = app.pattern_grip_value(&grip, view, cursor) {
                    app.set_pattern_grip(id, grip.label, value, mods_from(ui));
                }
            }
        }
        owned |= response.dragged() || response.hovered();
    }
    owned
}

/// Which way a world direction runs on screen, at `at`. A zero vector where the
/// line does not project -- behind the eye, or edge on.
pub(crate) fn screen_direction(view: &View, at: Vec3, dir: Vec3) -> egui::Vec2 {
    // A millimetre along the line is enough to take its bearing and short
    // enough that the answer is about the line at `at` rather than about where
    // it ends up.
    match (view.project(at), view.project(at + dir)) {
        (Some((a, _)), Some((b, _))) => b - a,
        _ => egui::Vec2::ZERO,
    }
}

/// Draw the selected pattern's lay-out grips: a leader line from the centre out
/// to each one, and a diamond on the grip itself. A span grip is drawn as an arc
/// round the ring it sets instead, since what it measures is the turn and not a
/// distance.
pub(crate) fn draw_pattern_grips(app: &App, painter: &egui::Painter, view: &View) {
    for grip in pattern_grips(app) {
        let Some((at, _)) = view.project(grip.at) else { continue };
        match grip.turn {
            Some((axis, zero, radius)) => {
                let tangent = axis.cross(zero);
                let mut arc: Vec<egui::Pos2> = Vec::new();
                // The whole way round to the grip, in one-degree steps, so the
                // arc shows the span the copies actually fill.
                let end = grip.at - grip.from;
                let span = end.dot(tangent).atan2(end.dot(zero)).to_degrees();
                let span = if span <= 0.0 { span + 360.0 } else { span };
                let steps = (span.abs().ceil() as usize).max(1);
                for i in 0..=steps {
                    let a = (span * i as f64 / steps as f64).to_radians();
                    let p = grip.from + zero * (radius * a.cos()) + tangent * (radius * a.sin());
                    if let Some((screen, _)) = view.project(p) {
                        arc.push(screen);
                    }
                }
                painter.add(egui::Shape::line(arc, egui::Stroke::new(1.0_f32, token::ACCENT.gamma_multiply(0.6))));
            }
            None => {
                if let Some((from, _)) = view.project(grip.from) {
                    painter.line_segment([from, at], egui::Stroke::new(1.0_f32, token::ACCENT.gamma_multiply(0.6)));
                }
            }
        }
        let r = 6.0;
        painter.add(egui::Shape::convex_polygon(
            vec![at + egui::vec2(0.0, -r), at + egui::vec2(r, 0.0), at + egui::vec2(0.0, r), at + egui::vec2(-r, 0.0)],
            token::ACCENT,
            egui::Stroke::NONE,
        ));
    }
}

/// Orange dots where the rule would put a copy, while the creation tool is open
/// on a pattern with nothing in it yet (issues 67, 96).
///
/// A pattern is *built* empty: the tool makes one out of the selection, and a
/// selection of nothing makes a pattern with nothing in it. That pattern has no
/// geometry, so there is nothing in the viewport to show what the rule is doing
/// and the window reads as a set of numbers with no effect. The rule still has
/// placements, and while the shape is missing they are the whole of what there
/// is to show. The original is the one every other copy is a copy *of*, so it is
/// the one drawn brightest.
pub(crate) fn draw_pattern_placements(app: &App, painter: &egui::Painter, view: &View) {
    // The pointer over a stage in the tool: ring each copy the rule has made by
    // the end of that stage (issue 79). Rings rather than dots, because the
    // shapes themselves are drawn under them -- a dot on a copy is lost in it,
    // and a ring round its origin reads as "this one" whatever the copy is.
    if let Some(stage) = app.pattern_tool_hover {
        for at in app.pattern_placements_through(stage) {
            let Some((screen, _)) = view.project(at) else { continue };
            painter.circle_stroke(screen, 6.0, egui::Stroke::new(2.0_f32, crate::theme::token::ACCENT));
        }
        return;
    }
    if !app.pattern_tool_is_empty() {
        return;
    }
    for (index, at) in app.pattern_placements().iter().enumerate() {
        // Orthographic, so there is no behind-the-camera to test for: every
        // placement lands somewhere, and the clip takes the ones off the
        // picture.
        let Some((screen, _)) = view.project(*at) else { continue };
        let (radius, colour) = if index == 0 {
            (5.0, crate::theme::token::ACCENT)
        } else {
            (3.5, crate::theme::token::ACCENT.gamma_multiply(0.7))
        };
        painter.circle_filled(screen, radius, colour);
    }
}

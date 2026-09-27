//! The handles a pattern is laid out by.

use super::*;
use crate::app::App;
use crate::theme::token;
use crate::view::View;
use simple3d_geom::Vec3;

/// The selected pattern's lay-out grips (issue 67): spacing and counts, radii, spans, rises. None
/// for a mirror or where every grip would sit on the origin.
pub(crate) fn pattern_grips(app: &App) -> Vec<crate::app::PatternGrip> {
    match app.primary() {
        Some(id) => app.pattern_grips(id),
        None => Vec::new(),
    }
}

/// Drag a pattern's grips to lay it out by eye; returns whether the pointer is theirs. Keyed by
/// label, since dragging "Copies" can add a "Spacing" grip and shift indices.
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
                // No cursor means "round", so a turn shows the grabbing hand and its arc shows the direction.
                Some(_) => egui::CursorIcon::Grabbing,
                // Other grips slide along a line, and the cursor follows that line's screen direction.
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

/// A world direction's screen direction at `at`; zero where the line does not project.
pub(crate) fn screen_direction(view: &View, at: Vec3, dir: Vec3) -> egui::Vec2 {
    // A millimetre along is enough for the bearing and local to `at`.
    match (view.project(at), view.project(at + dir)) {
        (Some((a, _)), Some((b, _))) => b - a,
        _ => egui::Vec2::ZERO,
    }
}

/// Draw the pattern's grips: a leader line and diamond each, or an arc round the ring for a span.
pub(crate) fn draw_pattern_grips(app: &App, painter: &egui::Painter, view: &View) {
    for grip in pattern_grips(app) {
        let Some((at, _)) = view.project(grip.at) else { continue };
        match grip.turn {
            Some((axis, zero, radius)) => {
                let tangent = axis.cross(zero);
                let mut arc: Vec<egui::Pos2> = Vec::new();
                // The full span in one-degree steps, showing what the copies fill.
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

/// Dots where the rule would put copies while the tool is open on an empty pattern (issues 67, 96),
/// since there is no geometry to show the rule; the original is brightest.
pub(crate) fn draw_pattern_placements(app: &App, painter: &egui::Painter, view: &View) {
    // Hovering a stage rings each copy made by its end (issue 79); rings stay visible over the shapes.
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
        // Orthographic, so every placement projects; the clip removes off-picture ones.
        let Some((screen, _)) = view.project(*at) else { continue };
        let (radius, colour) = if index == 0 {
            (5.0, crate::theme::token::ACCENT)
        } else {
            (3.5, crate::theme::token::ACCENT.gamma_multiply(0.7))
        };
        painter.circle_filled(screen, radius, colour);
    }
}

//! Drawing the plane and its grips.

use super::*;
use crate::app::App;
use crate::view::View;

/// Every section's frame and grips, over the finished image.
pub fn draw(app: &App, painter: &egui::Painter, view: &View) {
    if !app.scene.settings.section.enabled {
        return;
    }
    let count = app.scene.settings.section_count();
    for which in 0..count {
        draw_one(app, painter, view, which, count);
    }
}

/// One section's frame and grips; with several, each frame carries its tab number.
fn draw_one(app: &App, painter: &egui::Painter, view: &View, which: usize, count: usize) {
    let section = *app.scene.settings.section_at(which);
    let corners = frame(&section, app.evaluated.bounds);
    let screen: Vec<egui::Pos2> = corners.iter().filter_map(|&at| view.project(at).map(|(p, _)| p)).collect();
    if screen.len() < 4 {
        return;
    }
    // A reference, so quiet grey like the scene's bounding box, brightening to accent when hovered or moved.
    let grabbed = app.section_grab.is_some() && app.section_tab == which;
    let live = grabbed || app.section_hover.is_some_and(|(hovered, _)| hovered == which);
    let colour = match live {
        true => crate::theme::token::ACCENT,
        false => crate::theme::token::TEXT_LO,
    };
    // Grips at rest are a shade brighter than the frame, whose grey vanishes over the model.
    let mark = match live {
        true => crate::theme::token::ACCENT,
        false => crate::theme::token::TEXT_HI,
    };
    for (index, &from) in screen.iter().enumerate() {
        painter.line_segment([from, screen[(index + 1) % screen.len()]], egui::Stroke::new(1.0_f32, colour));
    }
    if count > 1 {
        painter.text(
            screen[3] + egui::vec2(4.0, -4.0),
            egui::Align2::LEFT_BOTTOM,
            (which + 1).to_string(),
            egui::FontId::proportional(crate::theme::font::LABEL),
            mark,
        );
    }
    for (index, at) in grips(&corners).into_iter().enumerate() {
        let Some((middle, _)) = view.project(at) else { continue };
        // Filled, since a hairline square vanishes over the model.
        painter.rect_filled(
            egui::Rect::from_center_size(middle, egui::Vec2::splat(GRIP)),
            2.0,
            crate::theme::token::SURFACE_1,
        );
        painter.rect_stroke(
            egui::Rect::from_center_size(middle, egui::Vec2::splat(GRIP)),
            2.0,
            egui::Stroke::new(1.5_f32, mark),
            egui::StrokeKind::Middle,
        );
        // Travel arrows on the hovered grip only, so the model is not covered in arrows.
        if app.section_hover != Some((which, index)) {
            continue;
        }
        let dir = crate::panel_viewport::screen_direction(view, at, travel(&section));
        if dir.length() > 1e-3 {
            let dir = dir / dir.length();
            for way in [1.0_f32, -1.0] {
                painter.line_segment(
                    [middle + dir * (GRIP * 0.5 * way), middle + dir * (GRIP * 1.3 * way)],
                    egui::Stroke::new(1.5_f32, mark),
                );
            }
        }
    }
}

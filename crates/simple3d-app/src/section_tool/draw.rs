//! Drawing the plane and its grips.

use super::*;
use crate::app::App;
use crate::view::View;

/// The plane's frame and its grip, over the finished image.
pub fn draw(app: &App, painter: &egui::Painter, view: &View) {
    let section = app.scene.settings.section;
    if !section.enabled {
        return;
    }
    let corners = frame(&section, app.evaluated.mesh.bounds());
    let screen: Vec<egui::Pos2> = corners.iter().filter_map(|&at| view.project(at).map(|(p, _)| p)).collect();
    if screen.len() < 4 {
        return;
    }
    // The frame is a reference rather than a selection, so it is drawn in the
    // quiet grey the scene's own bounding box uses -- and brightens to the
    // accent while the plane is under the pointer or being moved.
    let live = app.section_grab.is_some() || app.section_hover.is_some();
    let colour = match live {
        true => crate::theme::token::ACCENT,
        false => crate::theme::token::TEXT_LO,
    };
    // The grips are drawn a shade brighter than the frame when they are at
    // rest: they lie over the model as often as over the background, and the
    // frame's grey is the model's own grey -- a hairline in it disappears
    // exactly where the shape is.
    let mark = match live {
        true => crate::theme::token::ACCENT,
        false => crate::theme::token::TEXT_HI,
    };
    for (index, &from) in screen.iter().enumerate() {
        painter.line_segment([from, screen[(index + 1) % screen.len()]], egui::Stroke::new(1.0_f32, colour));
    }
    for (index, at) in grips(&corners).into_iter().enumerate() {
        let Some((middle, _)) = view.project(at) else { continue };
        // Filled, not outlined. Half of what a grip lies over is the model
        // itself, and a hairline square in a grey close to the model's own is
        // invisible exactly where it is most needed -- which is where the shape
        // is.
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
        // An arrow head each way out of the grip under the pointer, along the
        // line the plane travels: the frame says where the plane is, not which
        // way it slides. On that one grip alone, because five sets of arrows
        // over the model is a diagram of the control rather than the model.
        if app.section_hover != Some(index) {
            continue;
        }
        let dir = crate::panel_viewport::screen_direction(view, at, travel(section.axis()));
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

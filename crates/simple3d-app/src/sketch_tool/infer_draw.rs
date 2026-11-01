//! What a click would do, shown at the pointer before it is made: the constraints a drawing click
//! would infer as their marks beside the pointer, and the dimension being placed drawn where it
//! would go.

use super::*;
use crate::app::App;
use crate::icon;
use crate::theme::token;
use crate::view::View;

/// A mark's side beside the pointer, in pixels, and where the first one sits from it.
const MARK: f32 = 16.0;
const FROM_POINTER: egui::Vec2 = egui::vec2(16.0, -30.0);

pub(crate) fn draw(app: &App, painter: &egui::Painter, view: &View, cursor: egui::Pos2) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    if tool.draw.draws() {
        let Some((_, inferred)) = tool.drawing_click(view, cursor, super::grid_step(app, view)) else { return };
        for (k, infer) in inferred.iter().enumerate() {
            let centre = cursor + FROM_POINTER + egui::vec2((MARK + 2.0) * k as f32, 0.0);
            let rect = egui::Rect::from_center_size(centre, egui::Vec2::splat(MARK));
            painter.rect_filled(rect, 3.0, token::SURFACE_1.gamma_multiply(0.9));
            painter.rect_stroke(rect, 3.0, egui::Stroke::new(1.0_f32, token::ACCENT), egui::StrokeKind::Inside);
            icon::draw(painter, rect.shrink(2.5), infer.glyph(), token::ACCENT);
        }
        return;
    }
    if tool.draw != Draw::Dimension {
        return;
    }
    let Some(measure) = tool.measure() else { return };
    let over = tool.pick_at(view, cursor).filter(|p| !matches!(p, Pick::Constraint(_)));
    if over.is_some() && tool.selection.len() < 2 {
        return;
    }
    let Some(at) = tool.under(view, cursor) else { return };
    let Some((kind, from)) = tool.dimension_kind(measure, at) else { return };
    let label = Some([at[0] - from[0], at[1] - from[1]]);
    if let Some(shape) = super::dimension_draw::shape(tool, view, &kind, label, false) {
        super::glyphs::draw_preview(painter, &shape);
    }
}

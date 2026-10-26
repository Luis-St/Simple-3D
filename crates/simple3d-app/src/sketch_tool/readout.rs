//! Every line's length, written beside it, so the drawing reads as a drawing with sizes before
//! anything is dimensioned. A line held by a length dimension shows that instead.

use super::*;
use crate::theme::token;
use crate::view::View;
use simple3d_core::unit::format_length;
use simple3d_sketch::{ConstraintKind, Shape};

/// Lines shorter than this on screen, in pixels, have no room for their length.
const ROOM: f32 = 36.0;
/// How far from the line its length sits, in pixels.
const ASIDE: f32 = 14.0;

/// A small label at `at`, centred there, on the window colour so it reads over lines and the grid.
pub(crate) fn tag(painter: &egui::Painter, at: egui::Pos2, text: &str, colour: egui::Color32) {
    let font = egui::FontId::monospace(11.0);
    let galley = painter.layout_no_wrap(text.to_string(), font, colour);
    let rect = egui::Rect::from_center_size(at, galley.size()).expand(3.0);
    painter.rect_filled(rect, 3.0, token::SURFACE_1.gamma_multiply(0.85));
    painter.galley(rect.min + egui::vec2(3.0, 3.0), galley, colour);
}

/// Each line's length, on the side of it facing down and left on screen, as the constraint labels
/// hang up and to the right.
pub(crate) fn lengths(tool: &SketchTool, painter: &egui::Painter, view: &View) {
    let dimensioned = |p: PointId, q: PointId| {
        tool.sketch
            .constraints
            .iter()
            .any(|c| matches!(c.kind, ConstraintKind::Distance { a, b, .. } if (a, b) == (p, q) || (a, b) == (q, p)))
    };
    for curve in &tool.sketch.curves {
        let Shape::Line { a, b } = curve.shape else { continue };
        if dimensioned(a, b) {
            continue;
        }
        let (p, q) = (tool.sketch.at(a), tool.sketch.at(b));
        let (Some(sp), Some(sq)) = (tool.screen(view, p), tool.screen(view, q)) else { continue };
        let along = sq - sp;
        if along.length() < ROOM {
            continue;
        }
        let mut normal = egui::vec2(-along.y, along.x).normalized();
        if normal.y - normal.x < 0.0 {
            normal = -normal;
        }
        let at = sp + along * 0.5 + normal * ASIDE;
        let length = (q[0] - p[0]).hypot(q[1] - p[1]);
        let colour = if curve.construction { token::TEXT_LO.gamma_multiply(0.7) } else { token::TEXT_LO };
        tag(painter, at, &format_length(length, tool.unit), colour);
    }
}

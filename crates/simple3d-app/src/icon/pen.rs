//! The pen the glyphs are drawn with.

use super::*;
use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

/// Draw `glyph` centred in `rect`, in `colour`. The glyph is drawn inside the
/// largest square that fits, so a rectangle wider than it is tall still gets a
/// centred, undistorted icon.
pub fn draw(painter: &Painter, rect: Rect, glyph: Glyph, colour: Color32) {
    let side = rect.width().min(rect.height());
    let square = Rect::from_center_size(rect.center(), Vec2::splat(side));
    let width = (side / 16.0 * 1.5).max(1.0);
    let pen = Pen { painter, square, stroke: Stroke::new(width, colour), colour };
    paint(&pen, glyph);
}

pub(crate) struct Pen<'a> {
    pub(super) painter: &'a Painter,
    pub(super) square: Rect,
    pub(super) stroke: Stroke,
    pub(super) colour: Color32,
}

impl Pen<'_> {
    /// Unit coordinates: (0,0) is the top-left of the glyph's square, (1,1) the
    /// bottom-right. Every glyph below is written in this space, which is what
    /// makes them all agree on weight and margin.
    pub(super) fn at(&self, x: f32, y: f32) -> Pos2 {
        self.square.lerp_inside(egui::vec2(x, y))
    }

    pub(super) fn line(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(egui::Shape::line(points, self.stroke));
    }

    pub(super) fn closed(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(egui::Shape::closed_line(points, self.stroke));
    }

    pub(super) fn filled(&self, points: &[(f32, f32)], colour: Color32) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
    }

    pub(super) fn circle(&self, cx: f32, cy: f32, r: f32) {
        self.painter.circle_stroke(self.at(cx, cy), r * self.square.width(), self.stroke);
    }

    pub(super) fn disc(&self, cx: f32, cy: f32, r: f32, colour: Color32) {
        self.painter.circle_filled(self.at(cx, cy), r * self.square.width(), colour);
    }

    /// An ellipse, as a polyline -- the shape that says "this solid is round"
    /// when it is seen at an angle.
    pub(super) fn ellipse(&self, cx: f32, cy: f32, rx: f32, ry: f32, from: f32, to: f32) {
        let steps = 28;
        let points: Vec<(f32, f32)> = (0..=steps)
            .map(|i| {
                let t = from + (to - from) * i as f32 / steps as f32;
                (cx + rx * t.cos(), cy + ry * t.sin())
            })
            .collect();
        self.line(&points);
    }

    pub(super) fn arrow(&self, from: (f32, f32), to: (f32, f32)) {
        self.line(&[from, to]);
        let dir = egui::vec2(to.0 - from.0, to.1 - from.1).normalized() * 0.26;
        let side = egui::vec2(-dir.y, dir.x) * 0.62;
        self.filled(
            &[to, (to.0 - dir.x + side.x, to.1 - dir.y + side.y), (to.0 - dir.x - side.x, to.1 - dir.y - side.y)],
            self.colour,
        );
    }
}

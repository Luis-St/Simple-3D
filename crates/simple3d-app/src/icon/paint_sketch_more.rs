//! The glyphs of the sketch tools' newer variants and of the selection tools, in the same manner as
//! `paint_sketch`: what each puts down, its clicked points as dots, helper lines faint.

use super::paint_sketch::{dashed, faint, DOT};
use super::*;

/// Draw `glyph` if it is one of these; whether it was.
pub(super) fn paint(pen: &Pen<'_>, glyph: Glyph) -> bool {
    let dot = |x: f32, y: f32| pen.disc(x, y, DOT, pen.colour);
    match glyph {
        // The middle and one end.
        Glyph::SketchCentreLine => {
            pen.line(&[(0.12, 0.88), (0.88, 0.12)]);
            dot(0.5, 0.5);
            dot(0.88, 0.12);
        }
        // A turned rectangle from one side's ends and the far side.
        Glyph::SketchThreePointRectangle => {
            pen.closed(&[(0.1, 0.62), (0.56, 0.2), (0.9, 0.56), (0.44, 0.96)]);
            dot(0.1, 0.62);
            dot(0.56, 0.2);
            dot(0.9, 0.56);
        }
        Glyph::SketchRoundedRectangle => {
            let r = 0.16;
            let (l, t, rr, b) = (0.1, 0.22, 0.9, 0.78);
            pen.line(&[(l + r, t), (rr - r, t)]);
            pen.line(&[(l + r, b), (rr - r, b)]);
            pen.line(&[(l, t + r), (l, b - r)]);
            pen.line(&[(rr, t + r), (rr, b - r)]);
            pen.ellipse(l + r, t + r, r, r, TAU * 0.5, TAU * 0.75);
            pen.ellipse(rr - r, t + r, r, r, TAU * 0.75, TAU);
            pen.ellipse(rr - r, b - r, r, r, 0.0, TAU * 0.25);
            pen.ellipse(l + r, b - r, r, r, TAU * 0.25, TAU * 0.5);
        }
        // Both ends of a diameter.
        Glyph::SketchTwoPointCircle => {
            pen.circle(0.5, 0.5, 0.38);
            faint(pen, (0.12, 0.5), (0.88, 0.5));
            dot(0.12, 0.5);
            dot(0.88, 0.5);
        }
        // A line running on smoothly into an arc.
        Glyph::SketchTangentArc => {
            pen.line(&[(0.08, 0.84), (0.46, 0.84)]);
            pen.ellipse(0.46, 0.5, 0.34, 0.34, TAU * 0.25, -TAU * 0.25);
            dot(0.46, 0.84);
            dot(0.46, 0.16);
        }
        // A hexagon round a faint circle touching its sides.
        Glyph::SketchOuterPolygon => {
            let corners: Vec<(f32, f32)> = (0..6)
                .map(|i| {
                    let a = TAU * (i as f32 / 6.0 + 1.0 / 12.0);
                    (0.5 + 0.42 * a.cos(), 0.5 + 0.42 * a.sin())
                })
                .collect();
            pen.closed(&corners);
            let stroke = egui::Stroke::new(pen.stroke.width, pen.colour.gamma_multiply(0.4));
            pen.painter.circle_stroke(pen.at(0.5, 0.5), 0.36 * pen.square.width(), stroke);
            dot(0.5, 0.5);
        }
        Glyph::SketchSlot | Glyph::SketchOverallSlot => {
            obround(pen);
            if glyph == Glyph::SketchSlot {
                dot(0.3, 0.5);
                dot(0.7, 0.5);
            } else {
                dot(0.1, 0.5);
                dot(0.9, 0.5);
            }
        }
        // A slot bent round a faint centre arc.
        Glyph::SketchArcSlot => {
            let (cx, cy, r, w) = (0.5, 0.86, 0.5, 0.13);
            let (from, to) = (TAU * 0.6, TAU * 0.9);
            pen.ellipse(cx, cy, r + w, r + w, from, to);
            pen.ellipse(cx, cy, r - w, r - w, from, to);
            for (angle, back) in [(from, true), (to, false)] {
                let (x, y) = (cx + r * angle.cos(), cy + r * angle.sin());
                let (a0, a1) = if back { (angle + TAU * 0.5, angle + TAU) } else { (angle, angle + TAU * 0.5) };
                pen.ellipse(x, y, w, w, a0, a1);
            }
            let stroke = egui::Stroke::new(pen.stroke.width, pen.colour.gamma_multiply(0.4));
            let mid: Vec<egui::Pos2> = (0..=12)
                .map(|i| from + (to - from) * i as f32 / 12.0)
                .map(|a| pen.at(cx + r * a.cos(), cy + r * a.sin()))
                .collect();
            pen.painter.add(egui::Shape::line(mid, stroke));
            dot(cx, cy - 0.04);
        }
        Glyph::SketchEllipse => {
            pen.ellipse(0.5, 0.5, 0.42, 0.26, 0.0, TAU);
            faint(pen, (0.5, 0.5), (0.92, 0.5));
            dot(0.5, 0.5);
        }
        Glyph::SketchEllipseArc => {
            pen.ellipse(0.5, 0.62, 0.42, 0.3, TAU * 0.5, TAU);
            faint(pen, (0.5, 0.62), (0.92, 0.62));
            dot(0.5, 0.62);
            dot(0.08, 0.62);
        }
        // A line run on to a wall, the added piece faint.
        Glyph::SketchExtend => {
            pen.line(&[(0.86, 0.12), (0.86, 0.88)]);
            pen.line(&[(0.1, 0.5), (0.46, 0.5)]);
            dashed(pen, (0.52, 0.5), (0.84, 0.5), 2);
        }
        // A line with a gap where it is cut.
        Glyph::SketchSplit => {
            pen.line(&[(0.08, 0.72), (0.42, 0.5)]);
            pen.line(&[(0.58, 0.4), (0.92, 0.18)]);
            dot(0.5, 0.45);
        }
        // Two lines meeting in a straight cut corner.
        Glyph::SketchChamfer => {
            pen.line(&[(0.14, 0.9), (0.14, 0.44), (0.44, 0.14), (0.9, 0.14)]);
        }
        // An outline and the same outline further out.
        Glyph::SketchOffset => {
            pen.line(&[(0.32, 0.92), (0.32, 0.32), (0.92, 0.32)]);
            pen.line(&[(0.1, 0.92), (0.1, 0.1), (0.92, 0.1)]);
            faint(pen, (0.32, 0.32), (0.1, 0.1));
        }
        // A shape and its mirror image across a dashed line.
        Glyph::SketchMirror => {
            dashed(pen, (0.5, 0.06), (0.5, 0.94), 4);
            pen.closed(&[(0.4, 0.22), (0.4, 0.8), (0.1, 0.8)]);
            pen.closed(&[(0.6, 0.22), (0.6, 0.8), (0.9, 0.8)]);
        }
        Glyph::SketchLinearPattern => {
            for x in [0.1, 0.4, 0.7] {
                pen.closed(&[(x, 0.38), (x + 0.22, 0.38), (x + 0.22, 0.62), (x, 0.62)]);
            }
        }
        Glyph::SketchCircularPattern => {
            for k in 0..6 {
                let a = TAU * k as f32 / 6.0;
                pen.circle(0.5 + 0.34 * a.cos(), 0.5 + 0.34 * a.sin(), 0.1);
            }
            dot(0.5, 0.5);
        }
        // From a start to where it goes.
        Glyph::SketchMove => {
            pen.arrow((0.5, 0.5), (0.5, 0.08));
            pen.arrow((0.5, 0.5), (0.5, 0.92));
            pen.arrow((0.5, 0.5), (0.08, 0.5));
            pen.arrow((0.5, 0.5), (0.92, 0.5));
        }
        Glyph::SketchRotate => {
            pen.ellipse(0.5, 0.5, 0.36, 0.36, TAU * 0.6, TAU * 1.35);
            pen.arrow((0.62, 0.17), (0.4, 0.15));
            dot(0.5, 0.5);
        }
        // A small square and a bigger one from the same corner.
        Glyph::SketchScale => {
            pen.closed(&[(0.1, 0.9), (0.1, 0.1), (0.9, 0.1), (0.9, 0.9)]);
            faint(pen, (0.1, 0.9), (0.46, 0.54));
            pen.closed(&[(0.1, 0.9), (0.1, 0.54), (0.46, 0.54), (0.46, 0.9)]);
        }
        _ => return false,
    }
    true
}

/// A slot across the square.
fn obround(pen: &Pen<'_>) {
    let r = 0.2;
    pen.line(&[(0.3, 0.5 - r), (0.7, 0.5 - r)]);
    pen.line(&[(0.3, 0.5 + r), (0.7, 0.5 + r)]);
    pen.ellipse(0.7, 0.5, r, r, TAU * 0.75, TAU * 1.25);
    pen.ellipse(0.3, 0.5, r, r, TAU * 0.25, TAU * 0.75);
}

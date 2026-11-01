//! The sketch constraints' glyphs (issue 74), for their tool tiles and for the marks drawn beside
//! what each one holds: the shape of the rule, as CAD programs draw it.

use super::paint_sketch::{dashed, DOT};
use super::*;

/// Draw `glyph` if it is a constraint's; whether it was.
pub(super) fn paint(pen: &Pen<'_>, glyph: Glyph) -> bool {
    let dot = |x: f32, y: f32| pen.disc(x, y, DOT, pen.colour);
    match glyph {
        // Two lines meeting in one point.
        Glyph::ConstrainCoincident => {
            pen.line(&[(0.1, 0.84), (0.5, 0.5), (0.9, 0.7)]);
            pen.disc(0.5, 0.5, 0.15, pen.colour);
        }
        // A point sitting on an arc.
        Glyph::ConstrainOnCurve => {
            pen.ellipse(0.5, 0.95, 0.42, 0.42, TAU * 0.5, TAU);
            pen.disc(0.5, 0.53, 0.14, pen.colour);
        }
        // A point in the middle of a line, its ends marked.
        Glyph::ConstrainMidpoint => {
            pen.line(&[(0.08, 0.62), (0.92, 0.62)]);
            pen.line(&[(0.08, 0.48), (0.08, 0.76)]);
            pen.line(&[(0.92, 0.48), (0.92, 0.76)]);
            pen.filled(&[(0.5, 0.56), (0.36, 0.3), (0.64, 0.3)], pen.colour);
        }
        Glyph::ConstrainHorizontal => {
            pen.line(&[(0.12, 0.5), (0.88, 0.5)]);
            dot(0.12, 0.5);
            dot(0.88, 0.5);
        }
        Glyph::ConstrainVertical => {
            pen.line(&[(0.5, 0.12), (0.5, 0.88)]);
            dot(0.5, 0.12);
            dot(0.5, 0.88);
        }
        Glyph::ConstrainParallel => {
            pen.line(&[(0.14, 0.86), (0.58, 0.14)]);
            pen.line(&[(0.42, 0.86), (0.86, 0.14)]);
        }
        // Two lines square to each other, the corner marked.
        Glyph::ConstrainPerpendicular => {
            pen.line(&[(0.3, 0.1), (0.3, 0.86), (0.92, 0.86)]);
            pen.line(&[(0.3, 0.62), (0.54, 0.62), (0.54, 0.86)]);
        }
        // A line resting on a circle.
        Glyph::ConstrainTangent => {
            pen.circle(0.5, 0.62, 0.28);
            pen.line(&[(0.06, 0.34), (0.94, 0.34)]);
        }
        Glyph::ConstrainEqual => {
            pen.line(&[(0.14, 0.36), (0.86, 0.36)]);
            pen.line(&[(0.14, 0.64), (0.86, 0.64)]);
        }
        // Two marks facing each other across a dashed line.
        Glyph::ConstrainSymmetric => {
            dashed(pen, (0.5, 0.06), (0.5, 0.94), 4);
            pen.filled(&[(0.08, 0.3), (0.38, 0.5), (0.08, 0.7)], pen.colour);
            pen.filled(&[(0.92, 0.3), (0.62, 0.5), (0.92, 0.7)], pen.colour);
        }
        // A padlock.
        Glyph::ConstrainFix => {
            pen.ellipse(0.5, 0.46, 0.18, 0.2, TAU * 0.5, TAU);
            pen.line(&[(0.32, 0.46), (0.32, 0.52)]);
            pen.line(&[(0.68, 0.46), (0.68, 0.52)]);
            pen.filled(&[(0.22, 0.52), (0.78, 0.52), (0.78, 0.9), (0.22, 0.9)], pen.colour);
        }
        _ => return false,
    }
    true
}

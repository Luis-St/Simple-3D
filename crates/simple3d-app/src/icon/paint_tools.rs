//! The glyphs of the modelling tools added after the first set (issues 73, 88, 122).

use super::*;

pub(super) fn paint(pen: &Pen<'_>, glyph: Glyph) {
    match glyph {
        // A slab with its top face lifted along the arrow.
        Glyph::PushPull => {
            pen.closed(&[(0.10, 0.66), (0.50, 0.50), (0.90, 0.66), (0.50, 0.82)]);
            pen.line(&[(0.10, 0.66), (0.10, 0.76), (0.50, 0.92), (0.90, 0.76), (0.90, 0.66)]);
            pen.line(&[(0.50, 0.82), (0.50, 0.92)]);
            pen.arrow((0.50, 0.62), (0.50, 0.08));
        }
        // A square with its top right corner rounded off.
        Glyph::Round => {
            pen.line(&[(0.48, 0.14), (0.14, 0.14), (0.14, 0.86), (0.86, 0.86), (0.86, 0.52)]);
            pen.ellipse(0.48, 0.52, 0.38, 0.38, -TAU * 0.25, 0.0);
        }
        // An L-shaped outline and the lines it was swept along.
        Glyph::Extrusion => {
            let front = [(0.10, 0.40), (0.30, 0.40), (0.30, 0.70), (0.62, 0.70), (0.62, 0.90), (0.10, 0.90)];
            pen.closed(&front);
            let (dx, dy) = (0.26, -0.26);
            for &(x, y) in &front[..4] {
                pen.line(&[(x, y), (x + dx, y + dy)]);
            }
            pen.line(&[(0.10 + dx, 0.40 + dy), (0.30 + dx, 0.40 + dy), (0.30 + dx, 0.70 + dy)]);
            pen.line(&[(0.30 + dx, 0.70 + dy), (0.62 + dx, 0.70 + dy), (0.62 + dx, 0.90 + dy), (0.62, 0.90)]);
        }
        // A flag shape and its mirror image either side of a dashed line.
        Glyph::Mirror => {
            for i in 0..4 {
                let y = 0.08 + i as f32 * 0.23;
                pen.line(&[(0.50, y), (0.50, y + 0.13)]);
            }
            pen.closed(&[(0.40, 0.22), (0.40, 0.80), (0.08, 0.80)]);
            pen.closed(&[(0.60, 0.22), (0.60, 0.80), (0.92, 0.80)]);
        }
        _ => {}
    }
}

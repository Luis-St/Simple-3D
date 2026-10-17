//! The silhouettes of the shapes for printed parts (issue 123).

use super::*;
use std::f32::consts::PI;

pub(super) fn paint(pen: &Pen<'_>, glyph: Glyph) {
    match glyph {
        // A hexagon seen face on, with its hole: a standoff is the same seen from the side.
        Glyph::HexNut => {
            let corner = |i: u32| {
                let angle = PI / 3.0 * i as f32;
                (0.5 + 0.40 * angle.cos(), 0.5 + 0.40 * angle.sin())
            };
            pen.closed(&(0..6).map(corner).collect::<Vec<_>>());
            pen.circle(0.5, 0.5, 0.17);
        }
        Glyph::HexStandoff => {
            pen.closed(&[(0.22, 0.10), (0.78, 0.10), (0.78, 0.90), (0.22, 0.90)]);
            pen.line(&[(0.50, 0.10), (0.50, 0.90)]);
        }
        // The countersunk hole in section: the cone widening to the top over the shaft.
        Glyph::ScrewHole => {
            pen.line(&[(0.12, 0.14), (0.88, 0.14)]);
            pen.line(&[(0.14, 0.14), (0.36, 0.36), (0.36, 0.88), (0.64, 0.88), (0.64, 0.36), (0.86, 0.14)]);
        }
        // A rod with its thread drawn as slanted crests.
        Glyph::ThreadedRod => {
            pen.line(&[(0.30, 0.08), (0.30, 0.92)]);
            pen.line(&[(0.70, 0.08), (0.70, 0.92)]);
            for i in 0..5 {
                let y = 0.16 + 0.16 * i as f32;
                pen.line(&[(0.30, y + 0.06), (0.70, y - 0.06)]);
            }
        }
        // Teeth all round a bore.
        Glyph::Gear => {
            let teeth = 8;
            let mut outline = Vec::new();
            for i in 0..teeth {
                let middle = 2.0 * PI * i as f32 / teeth as f32;
                let step = PI / teeth as f32;
                for (angle, r) in [(-0.55, 0.30), (-0.25, 0.42), (0.25, 0.42), (0.55, 0.30)] {
                    let at = middle + angle * step;
                    outline.push((0.5 + r * at.cos(), 0.5 + r * at.sin()));
                }
            }
            pen.closed(&outline);
            pen.circle(0.5, 0.5, 0.11);
        }
        // A coil seen from the side.
        Glyph::Spring => {
            let mut points = vec![(0.5, 0.08)];
            for i in 0..5 {
                let y = 0.14 + 0.16 * i as f32;
                points.push((if i % 2 == 0 { 0.82 } else { 0.18 }, y + 0.08));
            }
            points.push((0.5, 0.92));
            pen.line(&points);
        }
        _ => {}
    }
}

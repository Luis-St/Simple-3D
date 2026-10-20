//! The silhouettes of the gears that mesh with the spur gear (issue 123).

use super::*;
use std::f32::consts::PI;

pub(super) fn paint(pen: &Pen<'_>, glyph: Glyph) {
    match glyph {
        // A rim with its teeth pointing inwards.
        Glyph::InternalGear => {
            pen.circle(0.5, 0.5, 0.42);
            let teeth = 12;
            let mut outline = Vec::new();
            for i in 0..teeth {
                let middle = 2.0 * PI * i as f32 / teeth as f32;
                let step = PI / teeth as f32;
                for (angle, r) in [(-0.55, 0.34), (-0.25, 0.24), (0.25, 0.24), (0.55, 0.34)] {
                    let at = middle + angle * step;
                    outline.push((0.5 + r * at.cos(), 0.5 + r * at.sin()));
                }
            }
            pen.closed(&outline);
        }
        // A bar seen from the side, its teeth along the top.
        Glyph::Rack => {
            let mut outline = vec![(0.08, 0.80), (0.08, 0.52)];
            for i in 0..4 {
                let x = 0.08 + 0.21 * i as f32;
                outline.extend([(x + 0.03, 0.52), (x + 0.07, 0.26), (x + 0.14, 0.26), (x + 0.18, 0.52)]);
            }
            outline.extend([(0.92, 0.52), (0.92, 0.80)]);
            pen.closed(&outline);
        }
        // A cone's frustum from the side, its teeth running up towards the apex.
        Glyph::BevelGear => {
            pen.closed(&[(0.08, 0.78), (0.92, 0.78), (0.70, 0.30), (0.30, 0.30)]);
            for (bottom, top) in [(0.30, 0.42), (0.50, 0.50), (0.70, 0.58)] {
                pen.line(&[(bottom, 0.78), (top, 0.30)]);
            }
        }
        // A shaft lying across, its thread drawn as broad slanted crests standing past it.
        Glyph::Worm => {
            pen.line(&[(0.04, 0.50), (0.14, 0.50)]);
            pen.line(&[(0.86, 0.50), (0.96, 0.50)]);
            pen.closed(&[(0.14, 0.36), (0.86, 0.36), (0.86, 0.64), (0.14, 0.64)]);
            for i in 0..4 {
                let x = 0.20 + 0.18 * i as f32;
                pen.closed(&[(x, 0.22), (x + 0.08, 0.22), (x + 0.12, 0.78), (x + 0.04, 0.78)]);
            }
        }
        // Pointed teeth round a wheel, a round seat for a roller between each two.
        Glyph::Sprocket => {
            let teeth = 8;
            let mut outline = Vec::new();
            for i in 0..teeth {
                let middle = 2.0 * PI * i as f32 / teeth as f32;
                let step = PI / teeth as f32;
                outline.push((0.5 + 0.43 * middle.cos(), 0.5 + 0.43 * middle.sin()));
                // The seat: a half circle sunk between this tooth and the next.
                let seat = middle + step;
                let (cx, cy) = (0.5 + 0.36 * seat.cos(), 0.5 + 0.36 * seat.sin());
                for k in 0..=6 {
                    let a = seat + 1.5 * PI - PI * k as f32 / 6.0;
                    outline.push((cx + 0.08 * a.cos(), cy + 0.08 * a.sin()));
                }
            }
            pen.closed(&outline);
            pen.circle(0.5, 0.5, 0.10);
        }
        _ => {}
    }
}

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
        _ => {}
    }
}

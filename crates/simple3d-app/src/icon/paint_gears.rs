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
        _ => {}
    }
}

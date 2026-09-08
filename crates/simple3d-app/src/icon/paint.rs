//! Every glyph, drawn.

use super::*;
use crate::theme::token;

pub(crate) fn paint(pen: &Pen<'_>, glyph: Glyph) {
    match glyph {
        Glyph::Move => {
            pen.arrow((0.5, 0.5), (0.5, 0.08));
            pen.arrow((0.5, 0.5), (0.5, 0.92));
            pen.arrow((0.5, 0.5), (0.08, 0.5));
            pen.arrow((0.5, 0.5), (0.92, 0.5));
        }
        Glyph::Rotate => {
            pen.ellipse(0.5, 0.52, 0.36, 0.36, 0.6, 0.6 + TAU * 0.78);
            pen.arrow((0.86, 0.30), (0.72, 0.12));
        }
        Glyph::Resize => {
            pen.closed(&[(0.14, 0.14), (0.62, 0.14), (0.62, 0.62), (0.14, 0.62)]);
            pen.line(&[(0.62, 0.86), (0.86, 0.86), (0.86, 0.62)]);
            pen.arrow((0.52, 0.52), (0.84, 0.84));
        }
        // A small square growing into a large one: the same shape, a factor
        // bigger. Resize's glyph shows a corner being pulled, which is the
        // other question -- what size, rather than how many times.
        Glyph::Scale => {
            pen.closed(&[(0.12, 0.56), (0.44, 0.56), (0.44, 0.88), (0.12, 0.88)]);
            pen.closed(&[(0.44, 0.12), (0.88, 0.12), (0.88, 0.56), (0.44, 0.56)]);
            pen.arrow((0.30, 0.72), (0.66, 0.34));
        }
        Glyph::Frame => {
            pen.line(&[(0.16, 0.84), (0.16, 0.16)]);
            pen.line(&[(0.16, 0.84), (0.84, 0.84)]);
            pen.line(&[(0.16, 0.84), (0.70, 0.34)]);
        }
        // A ruler laid across the tile with its ticks: the tool measures, it
        // does not change the model.
        Glyph::Measure => {
            pen.closed(&[(0.10, 0.62), (0.62, 0.10), (0.90, 0.38), (0.38, 0.90)]);
            for i in 1..4 {
                let t = i as f32 / 4.0;
                // Ticks stepping along the ruler and cutting across its width, the
                // middle one longer.
                let along = (0.10 + t * 0.52, 0.62 - t * 0.52);
                let depth = if i == 2 { 0.22 } else { 0.13 };
                pen.line(&[along, (along.0 + depth, along.1 + depth)]);
            }
        }
        // Repeats of one shape: a grid of small squares, which is what a pattern
        // node makes of its children.
        Glyph::Pattern => {
            for (cx, cy) in [(0.30, 0.30), (0.70, 0.30), (0.30, 0.70), (0.70, 0.70)] {
                pen.closed(&[
                    (cx - 0.16, cy - 0.16),
                    (cx + 0.16, cy - 0.16),
                    (cx + 0.16, cy + 0.16),
                    (cx - 0.16, cy + 0.16),
                ]);
            }
        }
        Glyph::Group => {
            pen.line(&[(0.34, 0.10), (0.16, 0.10), (0.16, 0.90), (0.34, 0.90)]);
            pen.line(&[(0.66, 0.10), (0.84, 0.10), (0.84, 0.90), (0.66, 0.90)]);
            pen.disc(0.5, 0.5, 0.1, pen.colour);
        }
        Glyph::Delete => {
            pen.line(&[(0.14, 0.26), (0.86, 0.26)]);
            pen.line(&[(0.38, 0.26), (0.38, 0.12), (0.62, 0.12), (0.62, 0.26)]);
            pen.line(&[(0.24, 0.26), (0.30, 0.90), (0.70, 0.90), (0.76, 0.26)]);
        }
        // The three booleans read as the same two circles, differing only in
        // what is filled -- which is the point.
        Glyph::Union => {
            pen.disc(0.36, 0.5, 0.28, pen.colour);
            pen.disc(0.64, 0.5, 0.28, pen.colour);
        }
        Glyph::Difference => {
            pen.disc(0.36, 0.5, 0.28, pen.colour);
            pen.disc(0.64, 0.5, 0.28, token::SURFACE_1);
            pen.circle(0.64, 0.5, 0.28);
        }
        Glyph::Intersection => {
            pen.circle(0.36, 0.5, 0.28);
            pen.circle(0.64, 0.5, 0.28);
            // The lens where the two overlap.
            let steps = 20;
            let mut lens: Vec<(f32, f32)> = Vec::new();
            for i in 0..=steps {
                let t = -1.05 + 2.1 * i as f32 / steps as f32;
                lens.push((0.36 + 0.28 * t.cos(), 0.5 + 0.28 * t.sin()));
            }
            for i in 0..=steps {
                let t = std::f32::consts::PI - 1.05 + 2.1 * i as f32 / steps as f32;
                lens.push((0.64 + 0.28 * t.cos(), 0.5 + 0.28 * t.sin()));
            }
            pen.filled(&lens, pen.colour);
        }
        Glyph::Shaded => {
            pen.filled(&[(0.5, 0.10), (0.90, 0.32), (0.90, 0.72), (0.5, 0.94), (0.10, 0.72), (0.10, 0.32)], pen.colour);
        }
        Glyph::ShadedEdges => {
            pen.filled(&[(0.5, 0.14), (0.86, 0.34), (0.86, 0.70), (0.5, 0.90), (0.14, 0.70), (0.14, 0.34)], pen.colour);
            pen.closed(&[(0.5, 0.14), (0.86, 0.34), (0.86, 0.70), (0.5, 0.90), (0.14, 0.70), (0.14, 0.34)]);
        }
        Glyph::Wireframe => {
            pen.closed(&[(0.5, 0.14), (0.86, 0.34), (0.86, 0.70), (0.5, 0.90), (0.14, 0.70), (0.14, 0.34)]);
            pen.line(&[(0.5, 0.14), (0.5, 0.52)]);
            pen.line(&[(0.5, 0.52), (0.14, 0.70)]);
            pen.line(&[(0.5, 0.52), (0.86, 0.70)]);
        }
        Glyph::Grid => {
            for i in 1..4 {
                let t = i as f32 / 4.0;
                pen.line(&[(0.1, 0.1 + t * 0.8), (0.9, 0.1 + t * 0.8)]);
                pen.line(&[(0.1 + t * 0.8, 0.1), (0.1 + t * 0.8, 0.9)]);
            }
            pen.closed(&[(0.1, 0.1), (0.9, 0.1), (0.9, 0.9), (0.1, 0.9)]);
        }
        // A solid with a slice taken off it: the body, the plane that took the
        // slice, and the hatch on the face the cut left behind.
        Glyph::Section => {
            pen.closed(&[(0.16, 0.24), (0.60, 0.24), (0.60, 0.84), (0.16, 0.84)]);
            pen.line(&[(0.60, 0.10), (0.60, 0.94)]);
            for i in 0..3 {
                let t = 0.32 + i as f32 * 0.17;
                pen.line(&[(0.22, t + 0.16), (0.38, t)]);
            }
        }
        Glyph::Eye => {
            pen.ellipse(0.5, 0.5, 0.42, 0.26, 0.0, TAU);
            pen.disc(0.5, 0.5, 0.13, pen.colour);
        }
        Glyph::EyeOff => {
            pen.ellipse(0.5, 0.5, 0.42, 0.26, 0.0, TAU);
            pen.line(&[(0.14, 0.86), (0.86, 0.14)]);
        }
        // The group's own mark is the deliberate exception to "no filled
        // pictograms": at 22 px row height a solid dot reads instantly.
        Glyph::Bracket => {
            pen.line(&[(0.44, 0.14), (0.26, 0.14), (0.26, 0.86), (0.44, 0.86)]);
            pen.line(&[(0.62, 0.34), (0.80, 0.34), (0.80, 0.66), (0.62, 0.66)]);
        }
        Glyph::Warning => {
            pen.closed(&[(0.5, 0.10), (0.94, 0.86), (0.06, 0.86)]);
            pen.line(&[(0.5, 0.38), (0.5, 0.62)]);
            pen.disc(0.5, 0.75, 0.05, pen.colour);
        }
        // A surface made of triangles, which is exactly what a mesh body is and
        // what tells it apart from the parametric shape it was converted from.
        Glyph::Mesh => {
            pen.closed(&[(0.5, 0.10), (0.90, 0.34), (0.90, 0.70), (0.5, 0.92), (0.10, 0.70), (0.10, 0.34)]);
            pen.line(&[(0.10, 0.34), (0.5, 0.52), (0.90, 0.34)]);
            pen.line(&[(0.5, 0.10), (0.5, 0.52), (0.5, 0.92)]);
            pen.line(&[(0.10, 0.70), (0.5, 0.52), (0.90, 0.70)]);
        }
        // The same solid the mesh mark draws, cut in two and the halves drawn
        // apart: one shape that is now several objects, which is the whole of
        // what a split is.
        Glyph::Split => {
            pen.line(&[(0.44, 0.12), (0.10, 0.32), (0.10, 0.68), (0.44, 0.88)]);
            pen.line(&[(0.44, 0.12), (0.44, 0.88)]);
            pen.line(&[(0.62, 0.12), (0.62, 0.88), (0.94, 0.68), (0.94, 0.32), (0.62, 0.12)]);
        }
        Glyph::Box => {
            pen.closed(&[(0.10, 0.34), (0.50, 0.14), (0.90, 0.34), (0.90, 0.72), (0.50, 0.92), (0.10, 0.72)]);
            pen.line(&[(0.10, 0.34), (0.50, 0.52), (0.90, 0.34)]);
            pen.line(&[(0.50, 0.52), (0.50, 0.92)]);
        }
        Glyph::RoundedBox => {
            let r = 0.14;
            pen.line(&[(0.14 + r, 0.14), (0.86 - r, 0.14)]);
            pen.line(&[(0.86, 0.14 + r), (0.86, 0.86 - r)]);
            pen.line(&[(0.86 - r, 0.86), (0.14 + r, 0.86)]);
            pen.line(&[(0.14, 0.86 - r), (0.14, 0.14 + r)]);
            pen.ellipse(0.14 + r, 0.14 + r, r, r, TAU * 0.5, TAU * 0.75);
            pen.ellipse(0.86 - r, 0.14 + r, r, r, TAU * 0.75, TAU);
            pen.ellipse(0.86 - r, 0.86 - r, r, r, 0.0, TAU * 0.25);
            pen.ellipse(0.14 + r, 0.86 - r, r, r, TAU * 0.25, TAU * 0.5);
        }
        Glyph::ChamferedBox => {
            // The rounded box's square with its corners cut off straight, so
            // the two shapes read as the same box with different edges.
            let c = 0.20;
            pen.closed(&[
                (0.14 + c, 0.14),
                (0.86 - c, 0.14),
                (0.86, 0.14 + c),
                (0.86, 0.86 - c),
                (0.86 - c, 0.86),
                (0.14 + c, 0.86),
                (0.14, 0.86 - c),
                (0.14, 0.14 + c),
            ]);
        }
        Glyph::Wedge => {
            pen.closed(&[(0.12, 0.84), (0.88, 0.84), (0.88, 0.22)]);
            pen.line(&[(0.12, 0.84), (0.30, 0.68), (0.88, 0.68)]);
        }
        Glyph::Prism => {
            pen.closed(&[(0.30, 0.12), (0.70, 0.12), (0.88, 0.42), (0.70, 0.72), (0.30, 0.72), (0.12, 0.42)]);
            pen.line(&[(0.12, 0.42), (0.12, 0.66), (0.30, 0.92), (0.70, 0.92), (0.88, 0.66), (0.88, 0.42)]);
        }
        Glyph::Sphere => {
            pen.circle(0.5, 0.5, 0.38);
            pen.ellipse(0.5, 0.5, 0.38, 0.15, 0.0, TAU);
        }
        Glyph::Cap => {
            pen.ellipse(0.5, 0.72, 0.40, 0.14, 0.0, TAU);
            pen.ellipse(0.5, 0.72, 0.40, 0.52, std::f32::consts::PI, TAU);
        }
        Glyph::Cylinder => {
            pen.ellipse(0.5, 0.24, 0.34, 0.13, 0.0, TAU);
            pen.line(&[(0.16, 0.24), (0.16, 0.76)]);
            pen.line(&[(0.84, 0.24), (0.84, 0.76)]);
            pen.ellipse(0.5, 0.76, 0.34, 0.13, 0.0, std::f32::consts::PI);
        }
        Glyph::Tube => {
            pen.ellipse(0.5, 0.26, 0.34, 0.13, 0.0, TAU);
            pen.ellipse(0.5, 0.26, 0.18, 0.07, 0.0, TAU);
            pen.line(&[(0.16, 0.26), (0.16, 0.74)]);
            pen.line(&[(0.84, 0.26), (0.84, 0.74)]);
            pen.ellipse(0.5, 0.74, 0.34, 0.13, 0.0, std::f32::consts::PI);
        }
        Glyph::Capsule => {
            pen.ellipse(0.5, 0.30, 0.28, 0.18, std::f32::consts::PI, TAU);
            pen.line(&[(0.22, 0.30), (0.22, 0.70)]);
            pen.line(&[(0.78, 0.30), (0.78, 0.70)]);
            pen.ellipse(0.5, 0.70, 0.28, 0.18, 0.0, std::f32::consts::PI);
        }
        Glyph::Torus => {
            pen.ellipse(0.5, 0.5, 0.42, 0.24, 0.0, TAU);
            pen.ellipse(0.5, 0.5, 0.18, 0.10, 0.0, TAU);
        }
        Glyph::Cone => {
            pen.ellipse(0.5, 0.76, 0.34, 0.13, 0.0, TAU);
            pen.line(&[(0.16, 0.76), (0.5, 0.12), (0.84, 0.76)]);
        }
        Glyph::Pyramid => {
            pen.closed(&[(0.5, 0.12), (0.90, 0.68), (0.50, 0.90), (0.10, 0.68)]);
            pen.line(&[(0.5, 0.12), (0.5, 0.90)]);
        }
        Glyph::Polyhedron => {
            pen.closed(&[(0.5, 0.08), (0.90, 0.34), (0.76, 0.86), (0.24, 0.86), (0.10, 0.34)]);
            pen.line(&[(0.5, 0.08), (0.5, 0.52)]);
            pen.line(&[(0.10, 0.34), (0.5, 0.52), (0.90, 0.34)]);
            pen.line(&[(0.24, 0.86), (0.5, 0.52), (0.76, 0.86)]);
        }
        Glyph::Plate => {
            pen.closed(&[(0.08, 0.46), (0.50, 0.28), (0.92, 0.46), (0.50, 0.66)]);
            pen.line(&[(0.08, 0.46), (0.08, 0.58), (0.50, 0.78), (0.92, 0.58), (0.92, 0.46)]);
            pen.line(&[(0.50, 0.66), (0.50, 0.78)]);
        }
        Glyph::Disc => {
            pen.ellipse(0.5, 0.48, 0.40, 0.20, 0.0, TAU);
            pen.line(&[(0.10, 0.48), (0.10, 0.60)]);
            pen.line(&[(0.90, 0.48), (0.90, 0.60)]);
            pen.ellipse(0.5, 0.60, 0.40, 0.20, 0.0, std::f32::consts::PI);
        }
        Glyph::Ring => {
            pen.ellipse(0.5, 0.50, 0.40, 0.20, 0.0, TAU);
            pen.ellipse(0.5, 0.50, 0.20, 0.10, 0.0, TAU);
        }
        Glyph::Slot => {
            // An obround seen face on: the shape of the hole it cuts.
            let r = 0.18;
            pen.line(&[(0.22, 0.50 - r), (0.78, 0.50 - r)]);
            pen.line(&[(0.22, 0.50 + r), (0.78, 0.50 + r)]);
            pen.ellipse(0.78, 0.50, r, r, TAU * 0.75, TAU * 1.25);
            pen.ellipse(0.22, 0.50, r, r, TAU * 0.25, TAU * 0.75);
        }
    }
}

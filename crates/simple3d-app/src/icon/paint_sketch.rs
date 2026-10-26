//! The sketch editor's tool glyphs (issue 74): each shows what the tool puts down, with the points
//! it is clicked at as dots, so a tool reads by how it is used.

use super::*;

/// A clicked point.
const DOT: f32 = 0.1;

/// Draw `glyph` if it is a sketch tool's; whether it was.
pub(super) fn paint(pen: &Pen<'_>, glyph: Glyph) -> bool {
    let dot = |x: f32, y: f32| pen.disc(x, y, DOT, pen.colour);
    match glyph {
        Glyph::SketchLine => {
            pen.line(&[(0.14, 0.86), (0.86, 0.14)]);
            dot(0.14, 0.86);
            dot(0.86, 0.14);
        }
        // Two opposite corners.
        Glyph::SketchRectangle => {
            pen.closed(&[(0.12, 0.24), (0.88, 0.24), (0.88, 0.76), (0.12, 0.76)]);
            dot(0.12, 0.76);
            dot(0.88, 0.24);
        }
        // The centre and a corner.
        Glyph::SketchCentreRectangle => {
            pen.closed(&[(0.12, 0.24), (0.88, 0.24), (0.88, 0.76), (0.12, 0.76)]);
            faint(pen, (0.5, 0.5), (0.88, 0.24));
            dot(0.5, 0.5);
            dot(0.88, 0.24);
        }
        // The centre and the rim.
        Glyph::SketchCircle => {
            pen.circle(0.5, 0.5, 0.38);
            faint(pen, (0.5, 0.5), (0.88, 0.5));
            dot(0.5, 0.5);
        }
        Glyph::SketchThreePointCircle => {
            pen.circle(0.5, 0.5, 0.38);
            for turn in [0.75_f32, 0.0833, 0.4167] {
                let a = turn * TAU;
                dot(0.5 + 0.38 * a.cos(), 0.5 + 0.38 * a.sin());
            }
        }
        // The centre, the start and the end.
        Glyph::SketchArc => {
            pen.ellipse(0.5, 0.66, 0.38, 0.38, TAU * 0.5, TAU);
            dot(0.5, 0.66);
            dot(0.12, 0.66);
            dot(0.88, 0.66);
        }
        // The start, the end and a point it bends through.
        Glyph::SketchThreePointArc => {
            pen.ellipse(0.5, 0.66, 0.38, 0.38, TAU * 0.5, TAU);
            dot(0.12, 0.66);
            dot(0.88, 0.66);
            dot(0.5, 0.28);
        }
        Glyph::SketchPolygon => {
            let corners: Vec<(f32, f32)> = (0..6)
                .map(|i| {
                    let a = TAU * (i as f32 / 6.0 + 1.0 / 12.0);
                    (0.5 + 0.4 * a.cos(), 0.5 + 0.4 * a.sin())
                })
                .collect();
            pen.closed(&corners);
            dot(0.5, 0.5);
        }
        // A wave through the points it is clicked at.
        Glyph::SketchSpline => {
            pen.line(&wave(0.28));
            for (x, y) in [(0.1, 0.5), (0.3, 0.22), (0.7, 0.78), (0.9, 0.5)] {
                dot(x, y);
            }
        }
        // A flatter wave under the faint outline of the points pulling on it.
        Glyph::SketchControlSpline => {
            pen.line(&wave(0.2));
            let pulls = [(0.1, 0.5), (0.3, 0.1), (0.7, 0.9), (0.9, 0.5)];
            for pair in pulls.windows(2) {
                faint(pen, pair[0], pair[1]);
            }
            for (x, y) in pulls {
                dot(x, y);
            }
        }
        Glyph::SketchPoint => {
            pen.line(&[(0.5, 0.18), (0.5, 0.38)]);
            pen.line(&[(0.5, 0.62), (0.5, 0.82)]);
            pen.line(&[(0.18, 0.5), (0.38, 0.5)]);
            pen.line(&[(0.62, 0.5), (0.82, 0.5)]);
            pen.disc(0.5, 0.5, 0.1, pen.colour);
        }
        Glyph::SketchSelect => {
            pen.closed(&[(0.26, 0.1), (0.26, 0.8), (0.43, 0.65), (0.56, 0.92), (0.67, 0.87), (0.54, 0.6), (0.76, 0.6)]);
        }
        // A line between two crossing ones, the piece between them faded out.
        Glyph::SketchTrim => {
            pen.line(&[(0.3, 0.12), (0.3, 0.88)]);
            pen.line(&[(0.7, 0.12), (0.7, 0.88)]);
            pen.line(&[(0.06, 0.5), (0.3, 0.5)]);
            pen.line(&[(0.7, 0.5), (0.94, 0.5)]);
            faint(pen, (0.3, 0.5), (0.7, 0.5));
        }
        // Two lines meeting in a rounded corner.
        Glyph::SketchFillet => {
            pen.line(&[(0.14, 0.9), (0.14, 0.52)]);
            pen.ellipse(0.52, 0.52, 0.38, 0.38, TAU * 0.5, TAU * 0.75);
            pen.line(&[(0.52, 0.14), (0.9, 0.14)]);
        }
        // A measured span between two extension lines.
        Glyph::SketchDimension => {
            pen.line(&[(0.12, 0.24), (0.12, 0.84)]);
            pen.line(&[(0.88, 0.24), (0.88, 0.84)]);
            pen.arrow((0.5, 0.56), (0.14, 0.56));
            pen.arrow((0.5, 0.56), (0.86, 0.56));
        }
        Glyph::SketchRegions => {
            let square = [(0.14, 0.14), (0.86, 0.14), (0.86, 0.86), (0.14, 0.86)];
            pen.filled(&square, pen.colour.gamma_multiply(0.35));
            pen.closed(&square);
        }
        // A dashed line with the turn about it.
        Glyph::SketchAxis => {
            dashed(pen, (0.5, 0.04), (0.5, 0.96), 4);
            pen.ellipse(0.5, 0.5, 0.4, 0.16, 0.35, 0.35 + TAU * 0.8);
            pen.arrow((0.82, 0.42), (0.9, 0.5));
        }
        // An outline and its dashed copy.
        Glyph::SketchCopyEdges => {
            pen.closed(&[(0.1, 0.1), (0.6, 0.1), (0.6, 0.6), (0.1, 0.6)]);
            for (from, to) in
                [((0.6, 0.4), (0.9, 0.4)), ((0.9, 0.4), (0.9, 0.9)), ((0.9, 0.9), (0.4, 0.9)), ((0.4, 0.9), (0.4, 0.6))]
            {
                dashed(pen, from, to, 2);
            }
        }
        _ => return false,
    }
    true
}

/// A helper line: the construction a shape is drawn from, or what a tool takes away.
fn faint(pen: &Pen<'_>, from: (f32, f32), to: (f32, f32)) {
    let stroke = egui::Stroke::new(pen.stroke.width, pen.colour.gamma_multiply(0.4));
    pen.painter.line_segment([pen.at(from.0, from.1), pen.at(to.0, to.1)], stroke);
}

/// A line drawn as `dashes` dashes with gaps between them.
fn dashed(pen: &Pen<'_>, from: (f32, f32), to: (f32, f32), dashes: usize) {
    let pieces = dashes * 2 - 1;
    let at = |t: f32| (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
    for dash in 0..dashes {
        let start = (dash * 2) as f32 / pieces as f32;
        let end = (dash * 2 + 1) as f32 / pieces as f32;
        pen.line(&[at(start), at(end)]);
    }
}

/// One full wave across the square, `height` either side of the middle.
fn wave(height: f32) -> Vec<(f32, f32)> {
    (0..=24)
        .map(|i| {
            let x = 0.1 + 0.8 * i as f32 / 24.0;
            (x, 0.5 - height * ((x - 0.1) / 0.8 * TAU).sin())
        })
        .collect()
}

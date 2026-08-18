//! Geometric line icons, drawn rather than loaded.
//!
//! A single self-contained binary is a hard constraint, so there is no icon
//! font and no SVG loader here: every glyph is a handful of strokes in a unit
//! square, scaled into whatever rectangle it is asked to fill. That also keeps
//! them crisp at fractional DPI scaling, which a bitmap sheet would not.
//!
//! The house style is the design's: 16 px, 1.5 px stroke, geometric, and no
//! filled pictograms except the object-type glyphs in the outliner.

use crate::theme::token;
use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

/// Every glyph the interface draws. Ordered by where it is used: tools first,
/// then outliner marks, then the primitive silhouettes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    // Tools.
    Move,
    Rotate,
    Resize,
    Frame,
    // Object actions.
    Group,
    Delete,
    // Booleans.
    Union,
    Difference,
    Intersection,
    // View.
    Shaded,
    ShadedEdges,
    Wireframe,
    Grid,
    Perspective,
    // Outliner marks.
    Eye,
    EyeOff,
    Bracket,
    Warning,
    // Primitive silhouettes.
    Box,
    RoundedBox,
    Wedge,
    Prism,
    Sphere,
    Cap,
    Cylinder,
    Tube,
    Capsule,
    Torus,
    Cone,
    Pyramid,
    Polyhedron,
    Plate,
    Disc,
    Ring,
}

impl Glyph {
    /// The silhouette for a primitive type, by its registry id. An unknown id
    /// falls back to the box rather than drawing nothing, so a primitive added
    /// to the registry without a glyph still has a tile that can be clicked.
    pub fn for_primitive(type_id: &str) -> Glyph {
        match type_id {
            "box" => Glyph::Box,
            "rounded_box" => Glyph::RoundedBox,
            "wedge" => Glyph::Wedge,
            "prism" => Glyph::Prism,
            "sphere" => Glyph::Sphere,
            "spherical_cap" => Glyph::Cap,
            "cylinder" => Glyph::Cylinder,
            "tube" => Glyph::Tube,
            "capsule" => Glyph::Capsule,
            "torus" => Glyph::Torus,
            "cone" => Glyph::Cone,
            "pyramid" | "regular_pyramid" => Glyph::Pyramid,
            "tetrahedron" | "octahedron" | "dodecahedron" | "icosahedron" => Glyph::Polyhedron,
            "plate" => Glyph::Plate,
            "disc" => Glyph::Disc,
            "ring" => Glyph::Ring,
            _ => Glyph::Box,
        }
    }
}

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

struct Pen<'a> {
    painter: &'a Painter,
    square: Rect,
    stroke: Stroke,
    colour: Color32,
}

impl Pen<'_> {
    /// Unit coordinates: (0,0) is the top-left of the glyph's square, (1,1) the
    /// bottom-right. Every glyph below is written in this space, which is what
    /// makes them all agree on weight and margin.
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.square.lerp_inside(egui::vec2(x, y))
    }

    fn line(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(egui::Shape::line(points, self.stroke));
    }

    fn closed(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(egui::Shape::closed_line(points, self.stroke));
    }

    fn filled(&self, points: &[(f32, f32)], colour: Color32) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
    }

    fn circle(&self, cx: f32, cy: f32, r: f32) {
        self.painter.circle_stroke(self.at(cx, cy), r * self.square.width(), self.stroke);
    }

    fn disc(&self, cx: f32, cy: f32, r: f32, colour: Color32) {
        self.painter.circle_filled(self.at(cx, cy), r * self.square.width(), colour);
    }

    /// An ellipse, as a polyline -- the shape that says "this solid is round"
    /// when it is seen at an angle.
    fn ellipse(&self, cx: f32, cy: f32, rx: f32, ry: f32, from: f32, to: f32) {
        let steps = 28;
        let points: Vec<(f32, f32)> = (0..=steps)
            .map(|i| {
                let t = from + (to - from) * i as f32 / steps as f32;
                (cx + rx * t.cos(), cy + ry * t.sin())
            })
            .collect();
        self.line(&points);
    }

    fn arrow(&self, from: (f32, f32), to: (f32, f32)) {
        self.line(&[from, to]);
        let dir = egui::vec2(to.0 - from.0, to.1 - from.1).normalized() * 0.26;
        let side = egui::vec2(-dir.y, dir.x) * 0.62;
        self.filled(
            &[to, (to.0 - dir.x + side.x, to.1 - dir.y + side.y), (to.0 - dir.x - side.x, to.1 - dir.y - side.y)],
            self.colour,
        );
    }
}

const TAU: f32 = std::f32::consts::TAU;

fn paint(pen: &Pen<'_>, glyph: Glyph) {
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
        Glyph::Frame => {
            pen.line(&[(0.16, 0.84), (0.16, 0.16)]);
            pen.line(&[(0.16, 0.84), (0.84, 0.84)]);
            pen.line(&[(0.16, 0.84), (0.70, 0.34)]);
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
        Glyph::Perspective => {
            pen.closed(&[(0.30, 0.24), (0.70, 0.16), (0.70, 0.84), (0.30, 0.76)]);
            pen.line(&[(0.08, 0.12), (0.30, 0.24)]);
            pen.line(&[(0.08, 0.88), (0.30, 0.76)]);
            pen.line(&[(0.92, 0.06), (0.70, 0.16)]);
            pen.line(&[(0.92, 0.94), (0.70, 0.84)]);
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
    }
}

/// An icon-only button, the tool rail's unit of currency. `active` fills it with
/// the accent rather than tinting it, so which tool is in force is legible at a
/// glance and not a shade of guesswork.
pub fn button(ui: &mut egui::Ui, glyph: Glyph, size: f32, active: bool, enabled: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(size), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let painter = ui.painter();
    let radius = egui::CornerRadius::same(3);
    let colour = if !enabled {
        token::TEXT_LO.gamma_multiply(0.45)
    } else if active {
        painter.rect_filled(rect, radius, token::ACCENT);
        token::SURFACE_0
    } else if response.hovered() {
        painter.rect_filled(rect, radius, token::SURFACE_3);
        token::TEXT_HI
    } else {
        token::TEXT_LO
    };
    draw(painter, rect.shrink(size * 0.22), glyph, colour);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use scadstudio_core::primitive;

    #[test]
    fn every_primitive_in_the_registry_has_a_silhouette() {
        // A tile with no glyph would be an unclickable blank in the palette, so
        // check the mapping covers the registry rather than falling back.
        for spec in primitive::REGISTRY {
            let glyph = Glyph::for_primitive(spec.type_id);
            if spec.type_id != "box" {
                assert_ne!(glyph, Glyph::Box, "{} fell back to the box silhouette", spec.type_id);
            }
        }
    }

    #[test]
    fn an_unknown_type_still_gets_something_to_draw() {
        assert_eq!(Glyph::for_primitive("not-a-primitive"), Glyph::Box);
    }
}

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
    Scale,
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
    // Outliner marks.
    Eye,
    EyeOff,
    Bracket,
    Warning,
    // Primitive silhouettes.
    Box,
    RoundedBox,
    ChamferedBox,
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
    Slot,
}

impl Glyph {
    /// The silhouette for a primitive type, by its registry id. An unknown id
    /// falls back to the box rather than drawing nothing, so a primitive added
    /// to the registry without a glyph still has a tile that can be clicked.
    pub fn for_primitive(type_id: &str) -> Glyph {
        match type_id {
            "box" => Glyph::Box,
            "rounded_box" => Glyph::RoundedBox,
            "chamfered_box" => Glyph::ChamferedBox,
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
            "slot" => Glyph::Slot,
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

/// The application's own icon, rasterized from the same drawing the packaged
/// icon files carry (`packaging/deb/net.simple3d.Simple3D.svg`): the box in the
/// accent colour on the darkest surface, with the rounded corners a desktop
/// icon has.
///
/// It is drawn here rather than loaded because the binary carries no files, and
/// because it has to exist at *runtime*: the window's icon -- what a taskbar,
/// an alt-tab list and a title bar show -- comes from what the application hands
/// the window system, not from the icon compiled into the .exe, and eframe
/// substitutes its own egui logo for any application that hands it nothing. That
/// is the "e" that was on the window instead of this.
pub fn app_icon(size: usize) -> egui::IconData {
    // The drawing is authored on a 64-unit square, so everything below is in
    // those units and scaled once, here.
    let scale = size as f64 / 64.0;
    let point = |x: f64, y: f64| (x * scale, y * scale);
    // The glyph, under the SVG's own translate(6.4 6.4) scale(0.8).
    let place = |x: f64, y: f64| point(6.4 + 0.8 * x, 6.4 + 0.8 * y);
    let hexagon = [(6.4, 21.76), (32.0, 8.96), (57.6, 21.76), (57.6, 46.08), (32.0, 58.88), (6.4, 46.08), (6.4, 21.76)];
    let mut strokes: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for pair in hexagon.windows(2) {
        strokes.push((place(pair[0].0, pair[0].1), place(pair[1].0, pair[1].1)));
    }
    for pair in [((6.4, 21.76), (32.0, 33.28)), ((32.0, 33.28), (57.6, 21.76)), ((32.0, 33.28), (32.0, 58.88))] {
        strokes.push((place(pair.0 .0, pair.0 .1), place(pair.1 .0, pair.1 .1)));
    }
    let half_stroke = 5.0 * 0.8 / 2.0 * scale;
    let radius = 12.0 * scale;
    let edge = size as f64;

    let background = token::SURFACE_0;
    let accent = token::ACCENT;
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let p = (x as f64 + 0.5, y as f64 + 0.5);
            // The tile: a rounded square, antialiased by its own distance field.
            let inside = coverage(-rounded_square_distance(p, edge, radius));
            if inside <= 0.0 {
                continue;
            }
            let distance = strokes.iter().map(|(a, b)| segment_distance(p, *a, *b)).fold(f64::INFINITY, f64::min);
            let ink = coverage(half_stroke - distance);
            let colour = blend(background, accent, ink);
            let offset = (y * size + x) * 4;
            rgba[offset] = colour.0;
            rgba[offset + 1] = colour.1;
            rgba[offset + 2] = colour.2;
            rgba[offset + 3] = (inside * 255.0).round() as u8;
        }
    }
    egui::IconData { rgba, width: size as u32, height: size as u32 }
}

/// One pixel's worth of antialiasing either side of an edge.
fn coverage(distance: f64) -> f64 {
    (distance + 0.5).clamp(0.0, 1.0)
}

fn blend(under: Color32, over: Color32, amount: f64) -> (u8, u8, u8) {
    let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * amount).round().clamp(0.0, 255.0) as u8;
    (mix(under.r(), over.r()), mix(under.g(), over.g()), mix(under.b(), over.b()))
}

/// Distance from a point to a line segment, which is what gives the strokes
/// their round caps and joins for free.
fn segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length_squared).clamp(0.0, 1.0)
    };
    (p.0 - (a.0 + dx * t)).hypot(p.1 - (a.1 + dy * t))
}

/// Signed distance to a square of side `edge` with corner radius `radius`,
/// negative inside it.
fn rounded_square_distance(p: (f64, f64), edge: f64, radius: f64) -> f64 {
    let half = edge / 2.0;
    let (qx, qy) = ((p.0 - half).abs() - (half - radius), (p.1 - half).abs() - (half - radius));
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    outside + qx.max(qy).min(0.0) - radius
}

#[cfg(test)]
mod tests {

    #[test]
    fn the_window_icon_is_the_box_in_the_accent_colour_on_a_rounded_tile() {
        // Issue 18: the window wore eframe's egui logo, because nothing set an
        // icon. What matters is that this produces one at all, and that it is
        // the drawing the packaged icon files carry rather than a blank square.
        let icon = app_icon(64);
        assert_eq!((icon.width, icon.height), (64, 64));
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);

        let pixel = |x: usize, y: usize| {
            let o = (y * 64 + x) * 4;
            (icon.rgba[o], icon.rgba[o + 1], icon.rgba[o + 2], icon.rgba[o + 3])
        };
        // The corners are rounded, so they are transparent; the middle is not.
        assert_eq!(pixel(0, 0).3, 0, "the tile's corner is not rounded");
        assert_eq!(pixel(32, 32).3, 255, "the middle of the tile is not opaque");

        let accent = token::ACCENT;
        let ink = icon
            .rgba
            .chunks_exact(4)
            .filter(|p| p[3] > 0 && p[0].abs_diff(accent.r()) < 24 && p[2].abs_diff(accent.b()) < 24)
            .count();
        assert!(ink > 200, "the box is not drawn in the accent colour: {ink} pixels of it");
        let ground = icon.rgba.chunks_exact(4).filter(|p| p[3] == 255 && p[0] == token::SURFACE_0.r()).count();
        assert!(ground > ink, "the tile is more line than ground: {ink} against {ground}");
    }

    #[test]
    fn the_icon_is_the_same_drawing_at_every_size() {
        // The same shape, scaled: the proportion of it that is ink cannot move
        // much between one size and the next, or the strokes are not scaling
        // with the tile.
        let inked = |size: usize| {
            let icon = app_icon(size);
            let accent = token::ACCENT;
            let ink = icon
                .rgba
                .chunks_exact(4)
                .filter(|p| p[3] > 0 && p[0].abs_diff(accent.r()) < 24 && p[2].abs_diff(accent.b()) < 24)
                .count() as f64;
            ink / (size * size) as f64
        };
        let (small, large) = (inked(32), inked(256));
        assert!((small - large).abs() < 0.05, "the drawing does not scale: {small} against {large}");
    }
    use super::*;
    use simple3d_core::primitive;

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

//! The application's own icon, rasterised for the window system.

use crate::theme::token;
use egui::Color32;

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
pub(crate) fn coverage(distance: f64) -> f64 {
    (distance + 0.5).clamp(0.0, 1.0)
}

pub(crate) fn blend(under: Color32, over: Color32, amount: f64) -> (u8, u8, u8) {
    let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * amount).round().clamp(0.0, 255.0) as u8;
    (mix(under.r(), over.r()), mix(under.g(), over.g()), mix(under.b(), over.b()))
}

/// Distance from a point to a line segment, which is what gives the strokes
/// their round caps and joins for free.
pub(crate) fn segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
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
pub(crate) fn rounded_square_distance(p: (f64, f64), edge: f64, radius: f64) -> f64 {
    let half = edge / 2.0;
    let (qx, qy) = ((p.0 - half).abs() - (half - radius), (p.1 - half).abs() - (half - radius));
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    outside + qx.max(qy).min(0.0) - radius
}

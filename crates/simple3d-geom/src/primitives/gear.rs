//! A spur gear with involute teeth (issue 123).

use super::fastener::extrude_ring;
use crate::mesh::Mesh;
use crate::revolve::extrude_frustum_polygon;
use std::f64::consts::{PI, TAU};

/// Points along each flank of a tooth.
const FLANK_POINTS: usize = 6;

/// A spur gear of `teeth` teeth of `module` (pitch diameter over teeth) with the given pressure
/// angle, `thickness` tall, with a round bore; a tooth points along +X. A bore of 0 leaves it
/// solid; one reaching the teeth is narrowed to leave a rim.
pub fn spur_gear_mesh(teeth: u32, module: f64, pressure_angle: f64, thickness: f64, bore: f64) -> Mesh {
    let outline = gear_outline(teeth.max(8), module, pressure_angle.clamp(10.0, 30.0).to_radians());
    let root = module * (teeth.max(8) as f64 / 2.0 - 1.25);
    let hole = bore.min(root * 2.0 * 0.9) / 2.0;
    if hole <= 1e-6 {
        return extrude_frustum_polygon(&outline, &outline, thickness);
    }
    // The bore met along each point's direction; the outline turns steadily, so they pair up.
    let inner: Vec<(f64, f64)> = outline
        .iter()
        .map(|&(x, y)| {
            let angle = y.atan2(x);
            (hole * angle.cos(), hole * angle.sin())
        })
        .collect();
    extrude_ring(&outline, &inner, thickness)
}

/// The gear's outline, counter-clockwise from the tooth on +X, every point further round than the last.
fn gear_outline(teeth: u32, module: f64, pressure: f64) -> Vec<(f64, f64)> {
    let z = teeth as f64;
    let pitch = module * z / 2.0;
    let base = pitch * pressure.cos();
    let tip = pitch + module;
    let root = pitch - 1.25 * module;
    let involute = |angle: f64| angle.tan() - angle;
    // A flank's angle off the tooth's middle at radius r: half a tooth at the pitch circle, less
    // the involute's turn from there. Inside the base circle the flank runs straight to the root.
    let off = |r: f64| {
        let at = (base / r.max(base)).clamp(-1.0, 1.0).acos();
        // Kept off the middle, where the flanks of a small gear with a steep angle would cross.
        (PI / (2.0 * z) + involute(pressure) - involute(at)).max(PI / (20.0 * z))
    };
    let start = root.max(base);
    let radii: Vec<f64> = (0..=FLANK_POINTS).map(|i| start + (tip - start) * i as f64 / FLANK_POINTS as f64).collect();
    let polar = |r: f64, angle: f64| (r * angle.cos(), r * angle.sin());
    let mut out = Vec::new();
    for tooth in 0..teeth {
        let middle = TAU * tooth as f64 / z;
        // The tip's middle first, so the tooth on +X has its point on the axis.
        out.push(polar(tip, middle));
        for &r in radii.iter().rev() {
            out.push(polar(r, middle + off(r)));
        }
        if root < base {
            out.push(polar(root, middle + off(base)));
        }
        // The gap's middle, then the next tooth's leading flank up to its tip.
        let next = middle + TAU / z;
        out.push(polar(root, (middle + next) / 2.0));
        if root < base {
            out.push(polar(root, next - off(base)));
        }
        for &r in &radii {
            out.push(polar(r, next - off(r)));
        }
    }
    out
}

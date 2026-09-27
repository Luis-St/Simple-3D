//! The 2D outlines a solid is swept or extruded from.

use std::f64::consts::TAU;

/// Points on an ellipse of radii (rx, ry) about the origin, from (rx, 0), counter-clockwise.
pub fn ring_outline(n: u32, rx: f64, ry: f64) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let theta = TAU * i as f64 / n as f64;
            (rx * theta.cos(), ry * theta.sin())
        })
        .collect()
}

/// A w x d rectangle with rounded vertical edges, each arc sampled with `corner_segments` extra
/// points (>= 1).
pub fn rounded_rect_outline(w: f64, d: f64, radius: f64, corner_segments: u32) -> Vec<(f64, f64)> {
    let r = radius.max(0.0).min(w / 2.0).min(d / 2.0);
    let seg = corner_segments.max(1);
    if r < 1e-9 {
        return vec![(w / 2.0, -d / 2.0), (w / 2.0, d / 2.0), (-w / 2.0, d / 2.0), (-w / 2.0, -d / 2.0)];
    }
    let corners = [
        (w / 2.0 - r, -(d / 2.0 - r), -std::f64::consts::FRAC_PI_2), // bottom-right
        (w / 2.0 - r, d / 2.0 - r, 0.0),                             // top-right
        (-(w / 2.0 - r), d / 2.0 - r, std::f64::consts::FRAC_PI_2),  // top-left
        (-(w / 2.0 - r), -(d / 2.0 - r), std::f64::consts::PI),      // bottom-left
    ];
    let mut out = Vec::with_capacity((4 * seg) as usize);
    for &(cx, cy, start) in &corners {
        for s in 0..seg {
            let angle = start + std::f64::consts::FRAC_PI_2 * (s as f64 / seg as f64);
            out.push((cx + r * angle.cos(), cy + r * angle.sin()));
        }
    }
    out
}

/// A w x d rectangle with 45-degree chamfered vertical edges, ordered like [`rounded_rect_outline`]
/// so the two are interchangeable.
pub fn chamfer_rect_outline(w: f64, d: f64, chamfer: f64) -> Vec<(f64, f64)> {
    let c = chamfer.max(0.0).min(w / 2.0).min(d / 2.0);
    if c < 1e-9 {
        return vec![(w / 2.0, -d / 2.0), (w / 2.0, d / 2.0), (-w / 2.0, d / 2.0), (-w / 2.0, -d / 2.0)];
    }
    let (hw, hd) = (w / 2.0, d / 2.0);
    vec![
        (hw - c, -hd),
        (hw, -hd + c),
        (hw, hd - c),
        (hw - c, hd),
        (-hw + c, hd),
        (-hw, hd - c),
        (-hw, -hd + c),
        (-hw + c, -hd),
    ]
}

/// Move every edge of a convex CCW outline inward by `inset` (a horizontal chamfer's outline). New
/// vertices are where offset edges cross: a true parallel offset, not a scale.
pub fn inset_convex_outline(outline: &[(f64, f64)], inset: f64) -> Vec<(f64, f64)> {
    let n = outline.len();
    if n < 3 || inset <= 0.0 {
        return outline.to_vec();
    }
    // Each edge as a point on it and its direction, after the offset.
    let offset_edge = |i: usize| -> ((f64, f64), (f64, f64)) {
        let (x0, y0) = outline[i];
        let (x1, y1) = outline[(i + 1) % n];
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = (dx * dx + dy * dy).sqrt().max(1e-12);
        // Interior is to the left of travel on a CCW outline.
        let (nx, ny) = (-dy / len, dx / len);
        ((x0 + nx * inset, y0 + ny * inset), (dx / len, dy / len))
    };
    (0..n)
        .map(|i| {
            let (prev_point, prev_dir) = offset_edge((i + n - 1) % n);
            let (point, dir) = offset_edge(i);
            let denominator = prev_dir.0 * dir.1 - prev_dir.1 * dir.0;
            if denominator.abs() < 1e-12 {
                // Collinear edges never cross; the offset point is already right.
                return point;
            }
            let (ex, ey) = (point.0 - prev_point.0, point.1 - prev_point.1);
            let t = (ex * dir.1 - ey * dir.0) / denominator;
            (prev_point.0 + prev_dir.0 * t, prev_point.1 + prev_dir.1 * t)
        })
        .collect()
}

/// One turn of an outline: a full ellipse at 360 degrees, otherwise a pie sector (centre then arc),
/// so the flat cut faces come from the same extrusion.
pub fn sector_outline(n: u32, rx: f64, ry: f64, sweep_deg: f64) -> Vec<(f64, f64)> {
    let sweep = sweep_deg.to_radians().clamp(0.0, TAU);
    if (TAU - sweep).abs() < 1e-9 {
        return ring_outline(n, rx, ry);
    }
    let n = n.max(3);
    let mut out = Vec::with_capacity(n as usize + 2);
    out.push((0.0, 0.0));
    for i in 0..=n {
        let theta = sweep * i as f64 / n as f64;
        out.push((rx * theta.cos(), ry * theta.sin()));
    }
    out
}

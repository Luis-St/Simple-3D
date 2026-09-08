//! The 2D outlines a solid is swept or extruded from.

use std::f64::consts::TAU;

/// Points on an ellipse of radii (rx, ry) around the origin, starting at
/// angle 0 (i.e. the point (rx, 0)), going counter-clockwise.
pub fn ring_outline(n: u32, rx: f64, ry: f64) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let theta = TAU * i as f64 / n as f64;
            (rx * theta.cos(), ry * theta.sin())
        })
        .collect()
}

/// A rectangle of size (w x d) with its 4 vertical edges rounded by `radius`,
/// each corner arc sampled with `corner_segments` extra points (>=1).
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

/// A rectangle of size (w x d) with its 4 vertical edges cut off at 45 degrees
/// by `chamfer`, in the same counter-clockwise order and starting corner as
/// [`rounded_rect_outline`], so the two are interchangeable.
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

/// Move every edge of a convex counter-clockwise outline inward by `inset`,
/// giving the outline a 45-degree chamfer would leave at a horizontal edge.
/// Each new vertex is where the two offset edges meeting at it cross, which is
/// what keeps the result a true parallel offset rather than a scaled copy --
/// scaling would move a long edge further than a short one.
pub fn inset_convex_outline(outline: &[(f64, f64)], inset: f64) -> Vec<(f64, f64)> {
    let n = outline.len();
    if n < 3 || inset <= 0.0 {
        return outline.to_vec();
    }
    // Each edge as a point on it plus its direction, after the offset.
    let offset_edge = |i: usize| -> ((f64, f64), (f64, f64)) {
        let (x0, y0) = outline[i];
        let (x1, y1) = outline[(i + 1) % n];
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = (dx * dx + dy * dy).sqrt().max(1e-12);
        // Interior is to the left of the direction of travel on a CCW outline.
        let (nx, ny) = (-dy / len, dx / len);
        ((x0 + nx * inset, y0 + ny * inset), (dx / len, dy / len))
    };
    (0..n)
        .map(|i| {
            let (prev_point, prev_dir) = offset_edge((i + n - 1) % n);
            let (point, dir) = offset_edge(i);
            let denominator = prev_dir.0 * dir.1 - prev_dir.1 * dir.0;
            if denominator.abs() < 1e-12 {
                // Two collinear edges never cross; the offset point is already
                // where the vertex belongs.
                return point;
            }
            let (ex, ey) = (point.0 - prev_point.0, point.1 - prev_point.1);
            let t = (ex * dir.1 - ey * dir.0) / denominator;
            (prev_point.0 + prev_dir.0 * t, prev_point.1 + prev_dir.1 * t)
        })
        .collect()
}

/// One turn of an outline: a full ellipse when `sweep_deg` reaches 360, and
/// otherwise a pie sector -- the centre point followed by the arc, so the two
/// straight cut faces come out of the same extrusion as the curved wall.
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

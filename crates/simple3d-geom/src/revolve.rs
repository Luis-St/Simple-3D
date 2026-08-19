//! Shared surface-of-revolution / extrusion helpers used by the primitive
//! generators. All of these build a "grid" of rings and triangulate the grid
//! with a winding convention validated to face outward:
//! for adjacent rings/profile-samples `V(j,i)`, `V(j2,i)`, `V(j2,i+1)`, `V(j,i+1)`
//! the two triangles `(V(j,i), V(j2,i), V(j2,i+1))` and
//! `(V(j,i), V(j2,i+1), V(j,i+1))` face outward when `j` sweeps CCW (viewed
//! from +Z) and the cross-section is traversed bottom-to-top / CCW as seen
//! in the (radius, z) half-plane.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
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

/// Extrude a (possibly tapering) polygon along Z from -h/2 to +h/2, capped at
/// both ends. `bottom` and `top` must have the same vertex count and are both
/// assumed centred on (0,0) and convex (or at least star-shaped around the
/// origin) so a fan cap is valid. Either outline may degenerate to all-zero
/// points (an apex), which is handled automatically via degenerate triangles.
pub fn extrude_frustum_polygon(bottom: &[(f64, f64)], top: &[(f64, f64)], h: f64) -> Mesh {
    debug_assert_eq!(bottom.len(), top.len());
    let n = bottom.len();
    let b: Vec<Vec3> = bottom.iter().map(|&(x, y)| Vec3::new(x, y, -h / 2.0)).collect();
    let t: Vec<Vec3> = top.iter().map(|&(x, y)| Vec3::new(x, y, h / 2.0)).collect();
    let mut mesh = Mesh::new();
    for i in 0..n {
        let i2 = (i + 1) % n;
        mesh.push_triangle(b[i], b[i2], t[i2]);
        mesh.push_triangle(b[i], t[i2], t[i]);
    }
    // A tapered end whose outline has collapsed to a point or a line (an
    // apex, or a wedge's sharp ridge) has zero area and no real cap face to
    // close -- the side walls already meet there. Only fan a genuine cap.
    let shoelace_area2 = |outline: &[(f64, f64)]| -> f64 {
        let n = outline.len();
        (0..n)
            .map(|i| {
                let (x0, y0) = outline[i];
                let (x1, y1) = outline[(i + 1) % n];
                x0 * y1 - x1 * y0
            })
            .sum()
    };
    let cb = Vec3::new(0.0, 0.0, -h / 2.0);
    let ct = Vec3::new(0.0, 0.0, h / 2.0);
    if shoelace_area2(bottom).abs() > 1e-9 {
        for i in 0..n {
            let i2 = (i + 1) % n;
            mesh.push_triangle(cb, b[i2], b[i]);
        }
    }
    if shoelace_area2(top).abs() > 1e-9 {
        for i in 0..n {
            let i2 = (i + 1) % n;
            mesh.push_triangle(ct, t[i], t[i2]);
        }
    }
    mesh
}

/// Revolve an open (radius, z) profile curve fully around Z. Ends whose
/// radius is ~0 close naturally at the axis; ends with radius > 0 are left
/// open unless `cap_start` / `cap_end` request a flat disc there.
pub fn revolve_open_profile(profile: &[(f64, f64)], segments: u32, cap_start: bool, cap_end: bool) -> Mesh {
    let n = segments.max(3);
    let m = profile.len();
    let mut mesh = Mesh::new();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|j| {
            let theta = TAU * j as f64 / n as f64;
            profile.iter().map(|&(r, z)| Vec3::new(r * theta.cos(), r * theta.sin(), z)).collect()
        })
        .collect();
    for j in 0..n as usize {
        let j2 = (j + 1) % n as usize;
        for i in 0..m - 1 {
            let a = rings[j][i];
            let b = rings[j2][i];
            let c = rings[j2][i + 1];
            let d = rings[j][i + 1];
            mesh.push_triangle(a, b, c);
            mesh.push_triangle(a, c, d);
        }
    }
    if cap_start && profile[0].0 > 1e-9 {
        let center = Vec3::new(0.0, 0.0, profile[0].1);
        for j in 0..n as usize {
            let j2 = (j + 1) % n as usize;
            mesh.push_triangle(center, rings[j2][0], rings[j][0]);
        }
    }
    if cap_end && profile[m - 1].0 > 1e-9 {
        let center = Vec3::new(0.0, 0.0, profile[m - 1].1);
        for j in 0..n as usize {
            let j2 = (j + 1) % n as usize;
            mesh.push_triangle(center, rings[j][m - 1], rings[j2][m - 1]);
        }
    }
    mesh
}

/// Revolve a *closed* (radius, z) loop (traversed CCW in the (r,z) plane,
/// e.g. an annulus rectangle or a tube's circular cross-section) around Z.
/// If `sweep_deg` is short of 360, the two open ends are fan-capped.
pub fn revolve_closed_profile(profile: &[(f64, f64)], segments: u32, sweep_deg: f64) -> Mesh {
    let n = segments.max(3);
    let sweep = sweep_deg.to_radians().clamp(0.0, TAU);
    let full = (TAU - sweep).abs() < 1e-9;
    let ring_count = if full { n } else { n + 1 };
    let m = profile.len();
    let mut mesh = Mesh::new();
    let rings: Vec<Vec<Vec3>> = (0..ring_count)
        .map(|j| {
            let theta = if full { TAU * j as f64 / n as f64 } else { sweep * j as f64 / n as f64 };
            profile.iter().map(|&(r, z)| Vec3::new(r * theta.cos(), r * theta.sin(), z)).collect()
        })
        .collect();
    let steps = if full { ring_count } else { ring_count - 1 };
    for j in 0..steps as usize {
        let j2 = if full { (j + 1) % ring_count as usize } else { j + 1 };
        for i in 0..m {
            let i2 = (i + 1) % m;
            let a = rings[j][i];
            let b = rings[j2][i];
            let c = rings[j2][i2];
            let d = rings[j][i2];
            mesh.push_triangle(a, b, c);
            mesh.push_triangle(a, c, d);
        }
    }
    if !full {
        let centroid =
            |ring: &[Vec3]| -> Vec3 { ring.iter().fold(Vec3::ZERO, |a, &b| a + b) * (1.0 / ring.len() as f64) };
        let start = &rings[0];
        let c0 = centroid(start);
        for i in 0..m {
            let i2 = (i + 1) % m;
            mesh.push_triangle(c0, start[i], start[i2]);
        }
        let end = &rings[ring_count as usize - 1];
        let c1 = centroid(end);
        for i in 0..m {
            let i2 = (i + 1) % m;
            mesh.push_triangle(c1, end[i2], end[i]);
        }
    }
    mesh
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

/// Extrude a stack of outlines, each at its own Z, capping the first and the
/// last. Every outline needs the same vertex count, and they have to be given
/// in increasing Z. This is [`extrude_frustum_polygon`] generalised to more
/// than two levels, which is what a horizontally chamfered solid needs: its
/// wall changes direction twice on the way up.
pub fn extrude_stack(levels: &[(Vec<(f64, f64)>, f64)]) -> Mesh {
    let mut mesh = Mesh::new();
    if levels.len() < 2 {
        return mesh;
    }
    let n = levels[0].0.len();
    let ring = |level: &(Vec<(f64, f64)>, f64)| -> Vec<Vec3> {
        level.0.iter().map(|&(x, y)| Vec3::new(x, y, level.1)).collect()
    };
    let rings: Vec<Vec<Vec3>> = levels.iter().map(ring).collect();
    for pair in rings.windows(2) {
        let (lower, upper) = (&pair[0], &pair[1]);
        for i in 0..n {
            let i2 = (i + 1) % n;
            mesh.push_triangle(lower[i], lower[i2], upper[i2]);
            mesh.push_triangle(lower[i], upper[i2], upper[i]);
        }
    }
    let area2 = |outline: &[(f64, f64)]| -> f64 {
        (0..outline.len())
            .map(|i| {
                let (x0, y0) = outline[i];
                let (x1, y1) = outline[(i + 1) % outline.len()];
                x0 * y1 - x1 * y0
            })
            .sum()
    };
    let first = &levels[0];
    if area2(&first.0).abs() > 1e-9 {
        let centre = Vec3::new(0.0, 0.0, first.1);
        for i in 0..n {
            mesh.push_triangle(centre, rings[0][(i + 1) % n], rings[0][i]);
        }
    }
    let last = levels.last().expect("checked non-empty");
    if area2(&last.0).abs() > 1e-9 {
        let centre = Vec3::new(0.0, 0.0, last.1);
        let top = rings.last().expect("checked non-empty");
        for i in 0..n {
            mesh.push_triangle(centre, top[i], top[(i + 1) % n]);
        }
    }
    mesh
}

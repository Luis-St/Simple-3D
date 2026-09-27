//! The world axes as things to snap to.

use super::*;
use simple3d_geom::{Mesh, Vec3};

/// Where the shown world axes pass through one body, as corner features (issue 78). Only the
/// crossings, since the stretch inside is cut out of the drawing.
pub fn axis_features(mesh: &Mesh, axes: [bool; 3]) -> Vec<Feature> {
    let Some((lo, hi)) = mesh.bounds() else { return Vec::new() };
    let mut out = Vec::new();
    for (axis, &shown) in axes.iter().enumerate() {
        if !shown {
            continue;
        }
        // The axis can only meet the body if it straddles zero on the other two coordinates.
        let others = [(axis + 1) % 3, (axis + 2) % 3];
        let range = |i: usize| (component(lo, i), component(hi, i));
        if others.iter().any(|&i| {
            let (l, h) = range(i);
            l > 1e-9 || h < -1e-9
        }) {
            continue;
        }
        let mut dir = Vec3::ZERO;
        set_component(&mut dir, axis, 1.0);

        // Every crossing along the line in order, so entry and exit pair up.
        let mut hits: Vec<f64> = Vec::new();
        for tri in &mesh.indices {
            let (a, b, c) =
                (mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]);
            if let Some(t) = line_triangle(Vec3::ZERO, dir, a, b, c) {
                hits.push(t);
            }
        }
        hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // Shared edges report twice and grazed faces report runs, so deduplicate.
        hits.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

        out.extend(hits.iter().map(|&t| Feature::point(dir * t, FeatureKind::AxisCrossing)));
    }
    out
}

/// The visible world axes as segments of length `reach`, catchable anywhere along them.
pub fn axis_lines(axes: [bool; 3], reach: f64) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    for (axis, &shown) in axes.iter().enumerate() {
        if !shown {
            continue;
        }
        let mut dir = Vec3::ZERO;
        set_component(&mut dir, axis, reach);
        out.push((dir * -1.0, dir));
    }
    out
}

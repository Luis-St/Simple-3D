//! The world axes as things to snap to.

use super::*;
use simple3d_geom::{Mesh, Vec3};

/// Where the world axes pass through one body, as features (issue 78).
///
/// The origin axes run through the model, and the place a measurement usually
/// wants -- where the axis leaves the body, where its centreline meets a face --
/// is often not a corner of the mesh at all, so nothing was there to catch. Each
/// crossing of the surface becomes a corner.
///
/// Only the crossings, and not the run between them: that stretch is inside the
/// material, where the renderer cuts the line out altogether, and a snap target
/// on a line nobody can see is a jump with no cause. The line ends at the
/// surface, and so does what can be caught on it.
///
/// `axes` says which of X, Y and Z are shown: an axis the user has turned off is
/// not on screen either.
pub fn axis_features(mesh: &Mesh, axes: [bool; 3]) -> Vec<Feature> {
    let Some((lo, hi)) = mesh.bounds() else { return Vec::new() };
    let mut out = Vec::new();
    for (axis, &shown) in axes.iter().enumerate() {
        if !shown {
            continue;
        }
        // The line is the axis itself, so it can only meet this body if the body
        // straddles zero on the other two coordinates.
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

        // Every crossing along the whole line, in order, so entry and exit come
        // out as a pair.
        let mut hits: Vec<f64> = Vec::new();
        for tri in &mesh.indices {
            let (a, b, c) =
                (mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]);
            if let Some(t) = line_triangle(Vec3::ZERO, dir, a, b, c) {
                hits.push(t);
            }
        }
        hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // A triangulated surface reports the shared edge of two triangles twice,
        // and a face the axis grazes reports a run of hits at one place.
        hits.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

        out.extend(hits.iter().map(|&t| Feature::point(dir * t, FeatureKind::AxisCrossing)));
    }
    out
}

/// The three world axes as segments long enough to cover the frame, for the
/// visible ones only.
///
/// An axis is a line, and a measurement along one -- "how far out along X is
/// this" -- wants to start anywhere on it, not only where it happens to meet a
/// body. `reach` is how far the axes are drawn from the origin, so nothing is
/// caught out where there is no line to see.
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

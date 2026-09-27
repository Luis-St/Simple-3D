//! Which stretches of an axis are inside the model and which are clear.

use simple3d_geom::{Mesh, Vec3};

/// `[a, b]` along an axis, cut at every boundary of `spans`, each piece saying whether it is inside.
/// Either end may be larger; pieces keep the asked direction so the fade stays put.
pub(crate) fn clip_spans(a: f64, b: f64, spans: &[(f64, f64)]) -> Vec<(f64, f64, bool)> {
    let (lo, hi) = (a.min(b), a.max(b));
    let mut cuts = vec![lo, hi];
    for &(start, end) in spans {
        for edge in [start, end] {
            if edge > lo && edge < hi {
                cuts.push(edge);
            }
        }
    }
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut pieces: Vec<(f64, f64, bool)> = Vec::with_capacity(cuts.len());
    for pair in cuts.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if to - from < 1e-9 {
            continue;
        }
        let middle = (from + to) / 2.0;
        pieces.push((from, to, spans.iter().any(|&(start, end)| middle > start && middle < end)));
    }
    if a > b {
        pieces.reverse();
        pieces = pieces.into_iter().map(|(from, to, flag)| (to, from, flag)).collect();
    }
    pieces
}

/// Where along `axis` one mesh's solids are, as spans with their body.
///
/// Sorted crossings are paired per body, since the scene is one mesh and pairing across bodies
/// would call the gap material; an odd one out is dropped. Depends only on the mesh, so it is
/// computed once per renderable (per frame it cost milliseconds), returning bodies, not tags.
pub(crate) fn axis_inside_spans(mesh: &Mesh, bodies: &[u16], axis: usize) -> Vec<((f64, f64), u16)> {
    // A solid that does not straddle zero on the other two coordinates cannot be on the axis.
    let Some((lo, hi)) = mesh.bounds() else { return Vec::new() };
    if (0..3).any(|other| other != axis && (component(lo, other) > 0.0 || component(hi, other) < 0.0)) {
        return Vec::new();
    }
    let mut crossings: std::collections::BTreeMap<u16, Vec<f64>> = std::collections::BTreeMap::new();
    for tri in &mesh.indices {
        let world = [mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]];
        if let Some(at) = axis_crossing(world, axis) {
            crossings.entry(bodies.get(tri[0] as usize).copied().unwrap_or(0)).or_default().push(at);
        }
    }
    let mut spans = Vec::new();
    for (body, mut at) in crossings {
        at.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // A crossing on a shared edge is found twice.
        at.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        spans.extend(at.chunks_exact(2).map(|pair| ((pair[0], pair[1]), body)));
    }
    spans
}

/// The point at `value` along the axis; the other coordinates are zero.
pub(crate) fn along(axis: usize, value: f64) -> Vec3 {
    match axis {
        0 => Vec3::new(value, 0.0, 0.0),
        1 => Vec3::new(0.0, value, 0.0),
        _ => Vec3::new(0.0, 0.0, value),
    }
}

pub(crate) fn component(v: Vec3, axis: usize) -> f64 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// Where the axis line pierces a triangle, as a coordinate along it: Moller-Trumbore against the
/// line, so crossings behind the origin count.
pub(crate) fn axis_crossing(world: [Vec3; 3], axis: usize) -> Option<f64> {
    let direction = along(axis, 1.0);
    let (edge1, edge2) = (world[1] - world[0], world[2] - world[0]);
    let pvec = direction.cross(edge2);
    let det = edge1.dot(pvec);
    // Edge-on to the axis: no crossing, and the maths is degenerate.
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tvec = -world[0];
    let u = tvec.dot(pvec) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = tvec.cross(edge1);
    let v = direction.dot(qvec) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(edge2.dot(qvec) * inv)
}

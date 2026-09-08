//! Which stretches of an axis are inside the model and which are clear.

use super::*;
use simple3d_geom::Vec3;

/// `[a, b]` -- a stretch of an axis, given as the coordinate along it -- cut at
/// every boundary of `spans`, each piece saying whether it lies inside one.
///
/// Either end may be the larger; the pieces come back in the order they were
/// asked for, so a line keeps its direction and the fade along it.
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

/// Where along `axis` one item's solids are, as spans of the coordinate along
/// it, each with the body it runs through.
///
/// A closed surface is crossed an even number of times, so the crossings sorted
/// and taken in pairs are the stretches inside it. They are paired *per body*,
/// because the mesh handed to the renderer is the whole scene at once and two
/// shapes on the same axis would otherwise pair across the gap between them --
/// which would call the empty space between two boxes "material". An odd count
/// means the line grazed an edge or the mesh is not closed; the odd one out is
/// dropped rather than turned into a span that runs to infinity.
pub(crate) fn axis_inside_spans(item: &Item<'_>, axis: usize, tag_base: u16) -> Vec<((f64, f64), u16)> {
    // The axis passes through the origin, so a solid that does not straddle zero
    // on the other two coordinates cannot be on it -- which is most of them, and
    // this is the whole mesh not looked at.
    let Some((lo, hi)) = item.renderable.mesh.bounds() else { return Vec::new() };
    if (0..3).any(|other| other != axis && (component(lo, other) > 0.0 || component(hi, other) < 0.0)) {
        return Vec::new();
    }
    let mut crossings: std::collections::BTreeMap<u16, Vec<f64>> = std::collections::BTreeMap::new();
    for (index, tri) in item.renderable.mesh.indices.iter().enumerate() {
        let world = [
            item.renderable.mesh.positions[tri[0] as usize],
            item.renderable.mesh.positions[tri[1] as usize],
            item.renderable.mesh.positions[tri[2] as usize],
        ];
        if let Some(at) = axis_crossing(world, axis) {
            crossings.entry(item.renderable.tag(index, tag_base)).or_default().push(at);
        }
    }
    let mut spans = Vec::new();
    for (tag, mut at) in crossings {
        at.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // A crossing on a shared edge is found twice, once for each triangle.
        at.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        spans.extend(at.chunks_exact(2).map(|pair| ((pair[0], pair[1]), tag)));
    }
    spans
}

/// The point on the axis at `value` along it. Every axis line passes through the
/// origin, in both styles, so the other two coordinates are zero.
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

/// Where the axis through the origin along `axis` pierces one triangle, as the
/// coordinate along that axis. Moller-Trumbore against the line rather than a
/// ray, so a crossing behind the origin is found as readily as one in front.
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

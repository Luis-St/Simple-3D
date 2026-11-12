//! Adding vertices under a dab, so a coarse mesh can take a stroke.

use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// At most this many rounds of splitting per call, a safety net: each round halves the longest
/// sides it reaches, so a side 4096 times too long is the most one call finishes.
const ROUNDS: usize = 12;

/// `mesh` with every triangle the dab reaches (any part within `radius` of `centre`) split until
/// no side of it is longer than `edge`, or `None` when nothing needed splitting. Stops short rather
/// than pass `budget` triangles.
///
/// Splits by longest sides ([`crate::shaping::split_longest`]), so the triangles keep their shape
/// rather than turning into slivers fanning out to far corners, which a raised stroke then showed as
/// creases. The surface stays closed and its shape does not change; the existing vertices keep their
/// numbers and new ones come after them. Each triangle's tag and source go to its pieces.
///
/// Also returns, for each new vertex in order, the two vertices it was put halfway between (either
/// may be new itself, from an earlier round, so always numbered lower), so whoever tracks where the
/// vertices were can place the new ones there too.
pub fn refine_within(
    mesh: &Mesh,
    edge: f64,
    centre: Vec3,
    radius: f64,
    budget: usize,
) -> Option<(Mesh, Vec<[u32; 2]>)> {
    if edge.is_nan() || edge <= 0.0 {
        return None;
    }
    let reached = |corners: [Vec3; 3]| distance_to_triangle(centre, corners) <= radius;
    let mut current: Option<Mesh> = None;
    let mut parents = Vec::new();
    for _ in 0..ROUNDS {
        let from = current.as_ref().unwrap_or(mesh);
        match crate::shaping::split_longest(from, &reached, edge, budget) {
            Some((split, added)) => {
                parents.extend(added);
                current = Some(split);
            }
            None => break,
        }
    }
    current.map(|mesh| (mesh, parents))
}

/// How far `p` is from the triangle: from the plane where `p` lies over it, else from its nearest side.
fn distance_to_triangle(p: Vec3, [a, b, c]: [Vec3; 3]) -> f64 {
    let normal = (b - a).cross(c - a);
    if normal.dot(normal) > 0.0 {
        let inside = [(a, b), (b, c), (c, a)].iter().all(|&(from, to)| (to - from).cross(p - from).dot(normal) >= 0.0);
        if inside {
            return (p - a).dot(normal).abs() / normal.length();
        }
    }
    [(a, b), (b, c), (c, a)].iter().map(|&(from, to)| distance_to_segment(p, from, to)).fold(f64::INFINITY, f64::min)
}

/// How far `p` is from the segment from `a` to `b`.
fn distance_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let along = b - a;
    let t = ((p - a).dot(along) / along.dot(along).max(1e-300)).clamp(0.0, 1.0);
    (a + along * t - p).length()
}

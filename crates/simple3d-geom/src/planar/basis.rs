//! Working in the plane: a 2D basis for it, and the small predicates that
//! read as geometry rather than arithmetic.

use crate::vec3::Vec3;

/// An orthonormal pair spanning the plane, so the region can be triangulated in
/// 2D with `normal` as the outward direction: a loop wound counter-clockwise
/// about `normal` has positive area.
pub(crate) fn plane_basis(normal: Vec3) -> (Vec3, Vec3) {
    let seed = if normal.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let u = seed.cross(normal).normalized();
    (u, normal.cross(u))
}

pub(crate) type Point = (f64, f64);

pub(crate) fn signed_area(points: &[Point]) -> f64 {
    let n = points.len();
    let mut sum = 0.0;
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        sum += a.0 * b.1 - b.0 * a.1;
    }
    sum * 0.5
}

/// Whether the vertex at loop slot `slot` still has another slot in the loop
/// naming the same mesh vertex -- the signature of a bridge seam.
pub(crate) fn duplicated(ids: &[u32], remaining: &[usize], slot: usize) -> bool {
    remaining.iter().filter(|&&s| ids[s] == ids[slot]).count() > 1
}

pub(crate) fn strictly_inside(p: Point, a: Point, b: Point, c: Point) -> bool {
    let side = |q: Point, r: Point| (r.0 - q.0) * (p.1 - q.1) - (r.1 - q.1) * (p.0 - q.0);
    // The loop is counter-clockwise, so an interior point is left of all three
    // edges. Points *on* an edge do not block: a bridge seam puts them there by
    // construction.
    side(a, b) > 1e-12 && side(b, c) > 1e-12 && side(c, a) > 1e-12
}

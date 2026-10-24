//! What a face's outline gives the shapes put down on it: whether a point is inside it, and where it
//! turns.

use crate::snap::point_key;
use simple3d_geom::Vec3;
use std::collections::HashMap;

/// Whether `at` is inside `outline`, in its plane, by counting the edges a ray from it crosses;
/// holes in a face are outline too, so a point in one is outside.
pub(super) fn encloses(outline: &[(Vec3, Vec3)], at: Vec3, normal: Vec3) -> bool {
    let helper = if normal.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let u = normal.cross(helper).normalized();
    let v = normal.cross(u);
    let flat = |p: Vec3| ((p - at).dot(u), (p - at).dot(v));
    let mut inside = false;
    for &(a, b) in outline {
        let ((ax, ay), (bx, by)) = (flat(a), flat(b));
        if (ay > 0.0) != (by > 0.0) && ax + (bx - ax) * (0.0 - ay) / (by - ay) > 0.0 {
            inside = !inside;
        }
    }
    inside
}

/// The points of `outline` where it turns, each with its two edges' directions away from it. A point
/// two edges meet at in a straight line is no corner.
pub(super) fn corners_of(outline: &[(Vec3, Vec3)]) -> Vec<(Vec3, [Vec3; 2])> {
    let mut ends: HashMap<(i64, i64, i64), (Vec3, Vec<Vec3>)> = HashMap::new();
    for &(a, b) in outline {
        for (from, to) in [(a, b), (b, a)] {
            ends.entry(point_key(from)).or_insert((from, Vec::new())).1.push((to - from).normalized());
        }
    }
    let mut corners: Vec<(Vec3, [Vec3; 2])> = ends
        .into_values()
        .filter_map(|(at, edges)| match edges[..] {
            [a, b] if a.cross(b).length() > 1e-6 => Some((at, [a, b])),
            _ => None,
        })
        .collect();
    // In a fixed order, so equally near corners break ties the same way every run.
    corners
        .sort_by(|a, b| (a.0.x, a.0.y, a.0.z).partial_cmp(&(b.0.x, b.0.y, b.0.z)).unwrap_or(std::cmp::Ordering::Equal));
    corners
}

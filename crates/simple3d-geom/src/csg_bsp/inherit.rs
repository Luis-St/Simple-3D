//! The colour of the walls a subtraction opens (issue 114).
//!
//! A cutter's faces left inside the base become the walls of the cut, but they are the base's
//! exposed inside, so they take the base's tag rather than the cutter's.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// `walls` retagged as the part of `base` each lies in: the base's own tag when it has one, else
/// that of the base face nearest each wall.
pub(super) fn inherit_tags(base: &Mesh, walls: impl Iterator<Item = Polygon>) -> Vec<Polygon> {
    let first = base.tag(0);
    let uniform = (0..base.indices.len()).all(|t| base.tag(t) == first);
    walls
        .map(|mut wall| {
            wall.tag = if uniform { first } else { nearest_tag(base, wall.centroid()) };
            wall
        })
        .collect()
}

fn nearest_tag(base: &Mesh, at: Vec3) -> u32 {
    let mut best = (f64::INFINITY, base.tag(0));
    for (index, &tri) in base.indices.iter().enumerate() {
        let [a, b, c] = base.corners(tri);
        let distance = (closest_on_triangle(at, a, b, c) - at).length();
        if distance < best.0 {
            best = (distance, base.tag(index));
        }
    }
    best.1
}

/// The point of triangle `a b c` nearest `p` (Ericson, Real-Time Collision Detection, 5.1.5).
fn closest_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

//! Rays and lines against triangles and boxes.

use crate::vec3::Vec3;

/// Where the line `origin + t * dir` crosses triangle `[a, b, c]`, as `t` of either sign, from
/// either side (Moeller-Trumbore). `slack` widens the triangle by that much in barycentric terms;
/// a line parallel to its plane never crosses.
pub fn line_triangle(origin: Vec3, dir: Vec3, [a, b, c]: [Vec3; 3], slack: f64) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(h) * inv;
    if u < -slack || u > 1.0 + slack {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < -slack || u + v > 1.0 + slack {
        return None;
    }
    Some(e2.dot(q) * inv)
}

/// Where the ray from `origin` along `dir` enters the box, if it does before `limit` (slab test).
/// A ray starting inside enters at zero.
pub fn ray_box(origin: Vec3, dir: Vec3, lo: Vec3, hi: Vec3, limit: f64) -> Option<f64> {
    let (mut near, mut far) = (0.0_f64, limit);
    for axis in 0..3 {
        let (o, d, l, h) = (origin.get(axis), dir.get(axis), lo.get(axis), hi.get(axis));
        if d.abs() < 1e-12 {
            if o < l || o > h {
                return None;
            }
            continue;
        }
        let (mut a, mut b) = ((l - o) / d, (h - o) / d);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        near = near.max(a);
        far = far.min(b);
        if near > far {
            return None;
        }
    }
    Some(near)
}

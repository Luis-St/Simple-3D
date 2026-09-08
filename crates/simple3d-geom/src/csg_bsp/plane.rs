//! The dividing plane of a BSP node, and the keys that decide when two of
//! them are the same plane.

use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Plane {
    pub(super) normal: Vec3,
    pub(super) w: f64,
}

impl Plane {
    pub(super) fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Option<Plane> {
        let n = (b - a).cross(c - a);
        if n.length() < 1e-12 {
            return None;
        }
        let n = n.normalized();
        Some(Plane { normal: n, w: n.dot(a) })
    }

    pub(super) fn flip(&self) -> Plane {
        Plane { normal: -self.normal, w: -self.w }
    }
}

/// A plane's identity, sign-independent: two faces of a solid that lie in the
/// same plane belong at the same BSP node whichever way they face.
pub(crate) fn unoriented_plane_key(p: &Plane) -> (i64, i64, i64, i64) {
    let flip = if p.normal.x.abs() > 1e-9 {
        p.normal.x < 0.0
    } else if p.normal.y.abs() > 1e-9 {
        p.normal.y < 0.0
    } else {
        p.normal.z < 0.0
    };
    let p = if flip { p.flip() } else { *p };
    plane_key(&p)
}

pub(crate) fn pos_key(p: Vec3) -> (i64, i64, i64) {
    let s = 1_000_000.0; // 1e-6 mm
    ((p.x * s).round() as i64, (p.y * s).round() as i64, (p.z * s).round() as i64)
}

pub(crate) fn plane_key(p: &Plane) -> (i64, i64, i64, i64) {
    let s = 1_000_000.0;
    (
        (p.normal.x * s).round() as i64,
        (p.normal.y * s).round() as i64,
        (p.normal.z * s).round() as i64,
        (p.w * s).round() as i64,
    )
}

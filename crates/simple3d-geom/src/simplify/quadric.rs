//! The quadric that says what a point costs.
//!
//! Garland and Heckbert's measure: the cost of a vertex at `v` is the sum of squared distances to
//! the original planes it stands for. It accumulates as a quadratic form, so the planes are
//! remembered after their triangles are gone: a long edge across a flat face collapses free, a
//! short one across a corner does not. Symmetric, so ten entries suffice.

use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Quadric {
    /// xx, xy, xz, xw, yy, yz, yw, zz, zw, ww: the upper triangle, row by row.
    m: [f64; 10],
}

impl Quadric {
    /// The quadric of the plane through `at` with normal `n`, weighted by triangle area so slivers do
    /// not outvote large flat faces.
    pub(crate) fn plane(n: Vec3, at: Vec3, weight: f64) -> Quadric {
        let d = -n.dot(at);
        let (a, b, c) = (n.x, n.y, n.z);
        Quadric {
            m: [
                a * a * weight,
                a * b * weight,
                a * c * weight,
                a * d * weight,
                b * b * weight,
                b * c * weight,
                b * d * weight,
                c * c * weight,
                c * d * weight,
                d * d * weight,
            ],
        }
    }

    pub(crate) fn add(&mut self, other: &Quadric) {
        for (into, from) in self.m.iter_mut().zip(other.m.iter()) {
            *into += *from;
        }
    }

    /// The cost of a vertex here, clamped at zero since rounding can go slightly negative and would
    /// sort before free collapses.
    pub(crate) fn error(&self, v: Vec3) -> f64 {
        let [xx, xy, xz, xw, yy, yz, yw, zz, zw, ww] = self.m;
        let e = xx * v.x * v.x
            + 2.0 * xy * v.x * v.y
            + 2.0 * xz * v.x * v.z
            + 2.0 * xw * v.x
            + yy * v.y * v.y
            + 2.0 * yz * v.y * v.z
            + 2.0 * yw * v.y
            + zz * v.z * v.z
            + 2.0 * zw * v.z
            + ww;
        e.max(0.0)
    }

    /// Where this quadric is smallest, if unique. `None` for a singular system is normal (on a flat
    /// face or straight crease); the caller then picks among the edge's ends and middle.
    pub(crate) fn optimal(&self, scale: f64) -> Option<Vec3> {
        let [xx, xy, xz, xw, yy, yz, yw, zz, zw, _] = self.m;
        let det = xx * (yy * zz - yz * yz) - xy * (xy * zz - yz * xz) + xz * (xy * yz - yy * xz);
        // Judged against the entries' scale, so the threshold works for microns and metres alike.
        if det.abs() <= 1e-10 * scale {
            return None;
        }
        let (b0, b1, b2) = (-xw, -yw, -zw);
        let x = b0 * (yy * zz - yz * yz) - xy * (b1 * zz - yz * b2) + xz * (b1 * yz - yy * b2);
        let y = xx * (b1 * zz - yz * b2) - b0 * (xy * zz - yz * xz) + xz * (xy * b2 - b1 * xz);
        let z = xx * (yy * b2 - b1 * yz) - xy * (xy * b2 - b1 * xz) + b0 * (xy * yz - yy * xz);
        Some(Vec3::new(x / det, y / det, z / det))
    }

    /// The entries' magnitude, for judging the determinant.
    pub(crate) fn scale(&self) -> f64 {
        let [xx, _, _, _, yy, _, _, zz, _, _] = self.m;
        (xx + yy + zz).abs().max(1e-12)
    }
}

//! The quadric that says what a point costs.
//!
//! Garland and Heckbert's measure: every triangle of the original surface is a
//! plane, and the error of putting a vertex at `v` is the sum of the squared
//! distances from `v` to the planes of the triangles that met at the vertices
//! it replaces. That sum is a quadratic form, so it can be *accumulated*: the
//! cost of a vertex that has already swallowed two hundred triangles is still
//! ten numbers, and the plane those triangles lay in is remembered long after
//! the triangles themselves are gone. That is what separates this from
//! collapsing the shortest edge -- a long edge across a flat face is free, and a
//! short one across a corner is not.
//!
//! Symmetric, so ten of the sixteen entries are the whole matrix.

use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Quadric {
    /// xx, xy, xz, xw, yy, yz, yw, zz, zw, ww -- the upper triangle, read
    /// across.
    m: [f64; 10],
}

impl Quadric {
    /// The quadric of the plane through `at` with normal `n`, weighted by the
    /// area of the triangle it came from.
    ///
    /// Area-weighted because a mesh is not tessellated evenly: a face split
    /// into a thousand slivers would otherwise outvote the big flat one beside
    /// it a thousand times over, and the simplification would eat the flat face
    /// to keep the slivers.
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

    /// What it costs to put a vertex here: never negative in exact arithmetic,
    /// and clamped because in floating point a point exactly on every plane can
    /// come out a hair below zero and a negative cost would sort before the
    /// free collapses.
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

    /// Where this quadric is smallest, when it has a single answer.
    ///
    /// `None` when the system is singular, which is the ordinary case rather
    /// than a failure: a vertex in the middle of a flat face is equally good
    /// anywhere on that plane, and one on a straight crease is equally good
    /// anywhere along it. The caller then picks between the ends of the edge
    /// and its middle, which are the three answers that cannot drift away from
    /// the surface.
    pub(crate) fn optimal(&self, scale: f64) -> Option<Vec3> {
        let [xx, xy, xz, xw, yy, yz, yw, zz, zw, _] = self.m;
        let det = xx * (yy * zz - yz * yz) - xy * (xy * zz - yz * xz) + xz * (xy * yz - yy * xz);
        // Scaled against the size of the matrix's own entries: a determinant of
        // 1e-9 is a well-conditioned system on a shape measured in microns and
        // noise on one measured in metres.
        if det.abs() <= 1e-10 * scale {
            return None;
        }
        let (b0, b1, b2) = (-xw, -yw, -zw);
        let x = b0 * (yy * zz - yz * yz) - xy * (b1 * zz - yz * b2) + xz * (b1 * yz - yy * b2);
        let y = xx * (b1 * zz - yz * b2) - b0 * (xy * zz - yz * xz) + xz * (xy * b2 - b1 * xz);
        let z = xx * (yy * b2 - b1 * yz) - xy * (xy * b2 - b1 * xz) + b0 * (xy * yz - yy * xz);
        Some(Vec3::new(x / det, y / det, z / det))
    }

    /// How big the entries are, for the determinant to be judged against.
    pub(crate) fn scale(&self) -> f64 {
        let [xx, _, _, _, yy, _, _, zz, _, _] = self.m;
        (xx + yy + zz).abs().max(1e-12)
    }
}

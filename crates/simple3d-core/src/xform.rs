//! Affine transforms: scale a node's own axes, rotate by its Euler angles, then
//! translate.
//!
//! A matrix rather than a stack of Euler triples, so nesting depth is unbounded
//! and composing a group's transform with its children's is exact. The rotation
//! is built by rotating the basis vectors with `Vec3::rotate_xyz_deg`, so it is
//! the same rotation the mesh transform performs by construction, rather than by
//! a hand-derived matrix product that could drift away from it.

#[cfg(test)]
mod tests;

use simple3d_geom::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xform {
    /// Row-major 3x3 rotation.
    pub m: [[f64; 3]; 3],
    pub t: Vec3,
}

impl Default for Xform {
    fn default() -> Self {
        Xform::IDENTITY
    }
}

impl Xform {
    pub const IDENTITY: Xform = Xform { m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: Vec3::ZERO };

    pub fn from_translation(t: Vec3) -> Xform {
        Xform { t, ..Xform::IDENTITY }
    }

    pub fn from_pos_rot(position: Vec3, rotation_deg: Vec3) -> Xform {
        Xform::from_pos_rot_scale(position, rotation_deg, Vec3::ONE)
    }

    /// Scale first, in the node's own axes, then rotate, then translate -- the
    /// order the mesh transform performs, so the two cannot disagree.
    pub fn from_pos_rot_scale(position: Vec3, rotation_deg: Vec3, scale: Vec3) -> Xform {
        let x = Vec3::new(1.0, 0.0, 0.0).rotate_xyz_deg(rotation_deg) * scale.x;
        let y = Vec3::new(0.0, 1.0, 0.0).rotate_xyz_deg(rotation_deg) * scale.y;
        let z = Vec3::new(0.0, 0.0, 1.0).rotate_xyz_deg(rotation_deg) * scale.z;
        Xform { m: [[x.x, y.x, z.x], [x.y, y.y, z.y], [x.z, y.z, z.z]], t: position }
    }

    /// Transform a point.
    pub fn point(&self, p: Vec3) -> Vec3 {
        self.vector(p) + self.t
    }

    /// Transform a direction: rotation only, no translation.
    pub fn vector(&self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        )
    }

    /// The world-space direction of local axis 0, 1 or 2.
    pub fn axis(&self, axis: usize) -> Vec3 {
        self.axis_vector(axis).normalized()
    }

    /// The same, unnormalised: its length is how many world units one unit along
    /// that local axis covers -- the accumulated scale, this node's and every
    /// ancestor's.
    pub fn axis_vector(&self, axis: usize) -> Vec3 {
        Vec3::new(self.m[0][axis], self.m[1][axis], self.m[2][axis])
    }

    /// `self` applied after `inner`.
    pub fn compose(&self, inner: &Xform) -> Xform {
        let mut m = [[0.0; 3]; 3];
        for r in 0..3 {
            for c in 0..3 {
                m[r][c] = (0..3).map(|k| self.m[r][k] * inner.m[k][c]).sum();
            }
        }
        Xform { m, t: self.point(inner.t) }
    }

    /// The general 3x3 inverse. Used to turn a world-space drag back into the
    /// parent-frame coordinates a node's `position` is stored in.
    ///
    /// Not the transpose: once a node can carry a scale the linear part is no
    /// longer orthonormal, and transposing it would divide by the scale where it
    /// should multiply. A degenerate matrix -- which only a zero scale can
    /// produce, and nothing lets one through -- inverts to the identity rotation
    /// with the translation undone, so a caller gets something finite rather than
    /// a field of NaN.
    pub fn inverse(&self) -> Xform {
        let m = self.m;
        let cofactor = |r: usize, c: usize| {
            let rows: Vec<usize> = (0..3).filter(|&i| i != r).collect();
            let cols: Vec<usize> = (0..3).filter(|&i| i != c).collect();
            let minor = m[rows[0]][cols[0]] * m[rows[1]][cols[1]] - m[rows[0]][cols[1]] * m[rows[1]][cols[0]];
            if (r + c).is_multiple_of(2) {
                minor
            } else {
                -minor
            }
        };
        let det = (0..3).map(|c| m[0][c] * cofactor(0, c)).sum::<f64>();
        let linear = if det.abs() < 1e-18 {
            Xform::IDENTITY.m
        } else {
            // The inverse is the transposed cofactor matrix over the determinant.
            let mut out = [[0.0; 3]; 3];
            for r in 0..3 {
                for c in 0..3 {
                    out[r][c] = cofactor(c, r) / det;
                }
            }
            out
        };
        let inv = Xform { m: linear, t: Vec3::ZERO };
        Xform { m: linear, t: -inv.vector(self.t) }
    }
}

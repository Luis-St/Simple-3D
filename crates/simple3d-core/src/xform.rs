//! Affine transforms: scale in the node's axes, rotate by its Euler angles, then translate.
//!
//! A matrix composes exactly at any nesting depth. Rotation is built by rotating basis vectors with
//! `Vec3::rotate_xyz_deg`, so it matches the mesh transform by construction.

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
    /// Feed the exact bits of the matrix, row by row, then of the translation to `hasher`, for keys
    /// that must change whenever the placement does.
    pub fn hash_bits<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        for number in self.m.as_flattened().iter().chain([self.t.x, self.t.y, self.t.z].iter()) {
            number.to_bits().hash(hasher);
        }
    }

    pub const IDENTITY: Xform = Xform { m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: Vec3::ZERO };

    pub fn from_translation(t: Vec3) -> Xform {
        Xform { t, ..Xform::IDENTITY }
    }

    pub fn from_pos_rot(position: Vec3, rotation_deg: Vec3) -> Xform {
        Xform::from_pos_rot_scale(position, rotation_deg, Vec3::ONE)
    }

    /// Scale, then rotate, then translate: the mesh transform's order.
    pub fn from_pos_rot_scale(position: Vec3, rotation_deg: Vec3, scale: Vec3) -> Xform {
        let x = Vec3::new(1.0, 0.0, 0.0).rotate_xyz_deg(rotation_deg) * scale.x;
        let y = Vec3::new(0.0, 1.0, 0.0).rotate_xyz_deg(rotation_deg) * scale.y;
        let z = Vec3::new(0.0, 0.0, 1.0).rotate_xyz_deg(rotation_deg) * scale.z;
        Xform { m: [[x.x, y.x, z.x], [x.y, y.y, z.y], [x.z, y.z, z.z]], t: position }
    }

    /// The Euler angles (X then Y then Z, in degrees, as `from_pos_rot` takes them) of the rotation
    /// taking the world axes to the orthonormal right-handed `x`, `y`, `z` (issue 73).
    pub fn rotation_of_axes(x: Vec3, y: Vec3, z: Vec3) -> Vec3 {
        // Columns are the turned axes: R = Rz * Ry * Rx.
        let (r00, r10, r20) = (x.x, x.y, x.z);
        let (r01, r11) = (y.x, y.y);
        let (r21, r22) = (y.z, z.z);
        let b = (-r20).clamp(-1.0, 1.0).asin();
        let (a, c) = if b.cos().abs() > 1e-9 {
            (r21.atan2(r22), r10.atan2(r00))
        } else {
            // Gimbal lock: X and Z turn about the same line, so X takes none of it.
            (0.0, (-r01).atan2(r11))
        };
        let tidy = |deg: f64| if deg.abs() < 1e-9 { 0.0 } else { deg };
        Vec3::new(tidy(a.to_degrees()), tidy(b.to_degrees()), tidy(c.to_degrees()))
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

    /// The same, unnormalised: its length is the accumulated scale along that axis.
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

    /// The general 3x3 inverse, turning world drags into parent-frame positions. Not the transpose,
    /// since scale makes the matrix non-orthonormal. A degenerate matrix inverts to identity rotation
    /// with the translation undone, never NaN.
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
            // The transposed cofactor matrix over the determinant.
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

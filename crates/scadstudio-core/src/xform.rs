//! Affine transforms: rotate by a node's Euler angles, then translate.
//!
//! A matrix rather than a stack of Euler triples, so nesting depth is unbounded
//! and composing a group's transform with its children's is exact. The rotation
//! is built by rotating the basis vectors with `Vec3::rotate_xyz_deg`, so it is
//! the same rotation the mesh transform performs by construction, rather than by
//! a hand-derived matrix product that could drift away from it.

use scadstudio_geom::Vec3;

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
    pub const IDENTITY: Xform =
        Xform { m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: Vec3::ZERO };

    pub fn from_translation(t: Vec3) -> Xform {
        Xform { t, ..Xform::IDENTITY }
    }

    pub fn from_pos_rot(position: Vec3, rotation_deg: Vec3) -> Xform {
        let x = Vec3::new(1.0, 0.0, 0.0).rotate_xyz_deg(rotation_deg);
        let y = Vec3::new(0.0, 1.0, 0.0).rotate_xyz_deg(rotation_deg);
        let z = Vec3::new(0.0, 0.0, 1.0).rotate_xyz_deg(rotation_deg);
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
        Vec3::new(self.m[0][axis], self.m[1][axis], self.m[2][axis]).normalized()
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

    /// The rotation part is orthonormal by construction, so the inverse is its
    /// transpose. Used to turn a world-space drag back into the parent-frame
    /// coordinates a node's `position` is stored in.
    pub fn inverse(&self) -> Xform {
        let mut m = [[0.0; 3]; 3];
        for r in 0..3 {
            for c in 0..3 {
                m[r][c] = self.m[c][r];
            }
        }
        let inv = Xform { m, t: Vec3::ZERO };
        Xform { m, t: -inv.vector(self.t) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-9
    }

    #[test]
    fn the_identity_leaves_points_alone() {
        assert!(close(Xform::IDENTITY.point(Vec3::new(1.0, 2.0, 3.0)), Vec3::new(1.0, 2.0, 3.0)));
    }

    #[test]
    fn it_matches_the_rotation_the_mesh_transform_performs() {
        // The whole point of building the matrix from rotated basis vectors.
        for rotation in [
            Vec3::new(30.0, 0.0, 0.0),
            Vec3::new(0.0, 45.0, 0.0),
            Vec3::new(0.0, 0.0, 90.0),
            Vec3::new(15.0, -40.0, 70.0),
        ] {
            let xf = Xform::from_pos_rot(Vec3::new(5.0, -2.0, 1.0), rotation);
            for p in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, -3.0, 4.0), Vec3::ZERO] {
                let expected = p.rotate_xyz_deg(rotation) + Vec3::new(5.0, -2.0, 1.0);
                assert!(close(xf.point(p), expected), "{rotation:?} {p:?}: {:?} vs {expected:?}", xf.point(p));
            }
        }
    }

    #[test]
    fn composing_matches_applying_one_after_the_other() {
        let outer = Xform::from_pos_rot(Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 90.0));
        let inner = Xform::from_pos_rot(Vec3::new(0.0, 5.0, 0.0), Vec3::new(45.0, 0.0, 0.0));
        let combined = outer.compose(&inner);
        for p in [Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0), Vec3::new(-4.0, 0.5, 7.0)] {
            assert!(close(combined.point(p), outer.point(inner.point(p))), "{p:?}");
        }
    }

    #[test]
    fn the_inverse_undoes_the_transform() {
        let xf = Xform::from_pos_rot(Vec3::new(3.0, -7.0, 2.0), Vec3::new(20.0, 35.0, -50.0));
        let inv = xf.inverse();
        for p in [Vec3::ZERO, Vec3::new(11.0, 2.0, -3.0)] {
            assert!(close(inv.point(xf.point(p)), p), "{p:?}");
            assert!(close(xf.point(inv.point(p)), p), "{p:?}");
        }
    }

    #[test]
    fn axes_are_unit_length_and_orthogonal() {
        let xf = Xform::from_pos_rot(Vec3::new(1.0, 2.0, 3.0), Vec3::new(12.0, -34.0, 56.0));
        for axis in 0..3 {
            assert!((xf.axis(axis).length() - 1.0).abs() < 1e-12);
        }
        assert!(xf.axis(0).dot(xf.axis(1)).abs() < 1e-12);
        assert!(xf.axis(1).dot(xf.axis(2)).abs() < 1e-12);
        // Right-handed: X cross Y is Z.
        assert!(close(xf.axis(0).cross(xf.axis(1)), xf.axis(2)));
    }

    #[test]
    fn an_unrotated_transform_has_the_world_axes() {
        let xf = Xform::from_translation(Vec3::new(5.0, 5.0, 5.0));
        assert!(close(xf.axis(0), Vec3::new(1.0, 0.0, 0.0)));
        assert!(close(xf.axis(1), Vec3::new(0.0, 1.0, 0.0)));
        assert!(close(xf.axis(2), Vec3::new(0.0, 0.0, 1.0)));
    }

    #[test]
    fn vectors_ignore_translation() {
        let xf = Xform::from_pos_rot(Vec3::new(100.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 90.0));
        let v = xf.vector(Vec3::new(1.0, 0.0, 0.0));
        assert!(close(v, Vec3::new(0.0, 1.0, 0.0)), "{v:?}");
    }
}

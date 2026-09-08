use simple3d_geom::Vec3;

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
    for rotation in
        [Vec3::new(30.0, 0.0, 0.0), Vec3::new(0.0, 45.0, 0.0), Vec3::new(0.0, 0.0, 90.0), Vec3::new(15.0, -40.0, 70.0)]
    {
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
fn a_scaled_transform_inverts_and_composes_like_any_other() {
    // The inverse used to be the transpose, which is only right while the
    // linear part is orthonormal. A scale is exactly what makes it not.
    let xf =
        Xform::from_pos_rot_scale(Vec3::new(3.0, -7.0, 2.0), Vec3::new(20.0, 35.0, -50.0), Vec3::new(2.0, 0.5, 3.0));
    let inv = xf.inverse();
    for p in [Vec3::ZERO, Vec3::new(11.0, 2.0, -3.0), Vec3::new(-1.0, 0.25, 8.0)] {
        assert!(close(inv.point(xf.point(p)), p), "{p:?} -> {:?}", inv.point(xf.point(p)));
        assert!(close(xf.point(inv.point(p)), p), "{p:?}");
    }

    // And the scale is applied in the node's own axes, before the rotation.
    let scaled = Xform::from_pos_rot_scale(Vec3::ZERO, Vec3::new(0.0, 0.0, 90.0), Vec3::new(2.0, 1.0, 1.0));
    assert!(close(scaled.point(Vec3::new(1.0, 0.0, 0.0)), Vec3::new(0.0, 2.0, 0.0)));

    let outer = Xform::from_pos_rot_scale(Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::new(3.0, 3.0, 3.0));
    let inner = Xform::from_pos_rot(Vec3::new(0.0, 2.0, 0.0), Vec3::ZERO);
    let combined = outer.compose(&inner);
    for p in [Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0)] {
        assert!(close(combined.point(p), outer.point(inner.point(p))), "{p:?}");
    }

    // A zero scale cannot be inverted; it must still return something finite.
    let flat = Xform::from_pos_rot_scale(Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO, Vec3::ZERO);
    assert!(flat.inverse().point(Vec3::ZERO).length().is_finite());
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

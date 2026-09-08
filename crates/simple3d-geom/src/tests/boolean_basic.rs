//! The three operations on shapes that meet squarely.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn boolean_difference_hole_in_plate_is_manifold() {
    let plate = primitives::box_mesh(40.0, 20.0, 4.0);
    let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 32).translated(Vec3::new(-8.0, 0.0, 0.0));
    let result = evaluate_boolean(BooleanOp::Difference, &[plate, hole]);
    assert_manifold("plate_with_hole", &result);
}

#[test]
pub(crate) fn boolean_union_touching_at_edge_is_manifold() {
    let a = primitives::box_mesh(10.0, 10.0, 10.0);
    let b = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(10.0, 0.0, 0.0));
    let result = evaluate_boolean(BooleanOp::Union, &[a, b]);
    assert_manifold("union_touching_faces", &result);
}

#[test]
pub(crate) fn boolean_intersection_and_hull() {
    let a = primitives::box_mesh(10.0, 10.0, 10.0);
    let b = primitives::cylinder_mesh(8.0, 8.0, 20.0, 24);
    let result = evaluate_boolean(BooleanOp::Intersection, &[a, b]);
    assert_manifold("intersection", &result);

    let a = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(-10.0, 0.0, 0.0));
    let b = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(10.0, 0.0, 0.0));
    let result = evaluate_boolean(BooleanOp::Hull, &[a, b]);
    assert_manifold("hull", &result);
}

#[test]
pub(crate) fn boolean_diff_simple_boxes() {
    let a = primitives::box_mesh(20.0, 20.0, 20.0);
    let b = primitives::box_mesh(10.0, 10.0, 30.0);
    let result = evaluate_boolean(BooleanOp::Difference, &[a, b]);
    assert_manifold("diff_simple_boxes", &result);
}

#[test]
pub(crate) fn boolean_intersect_simple_boxes() {
    let a = primitives::box_mesh(20.0, 20.0, 20.0);
    let b = primitives::box_mesh(10.0, 10.0, 30.0);
    let result = evaluate_boolean(BooleanOp::Intersection, &[a, b]);
    assert_manifold("intersect_simple_boxes", &result);
}

#[test]
pub(crate) fn boolean_subtract_coplanar_face_is_manifold() {
    // Spec acceptance criterion 5, first half: the tool's face lies exactly on
    // the base's face. This is the normal case, not an edge case -- every
    // pocket cut from a face produces it.
    let base = primitives::box_mesh(40.0, 20.0, 4.0);
    let tool = primitives::box_mesh(10.0, 10.0, 4.0); // top and bottom coplanar with base
    let result = evaluate_boolean(BooleanOp::Difference, &[base, tool]);
    assert_manifold("coplanar_subtract", &result);
}

#[test]
pub(crate) fn boolean_subtract_touching_at_edge_is_manifold() {
    // Criterion 5, second half: operands touch along a single edge, so the
    // subtraction removes nothing and the base must survive intact.
    let base = primitives::box_mesh(20.0, 20.0, 20.0);
    let tool = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(15.0, 15.0, 0.0));
    let result = evaluate_boolean(BooleanOp::Difference, &[base, tool]);
    assert_manifold("edge_touch_subtract", &result);
}

#[test]
pub(crate) fn boolean_subtract_disjoint_and_contained() {
    let base = primitives::box_mesh(20.0, 20.0, 20.0);
    let far = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(1000.0, 0.0, 0.0));
    let result = evaluate_boolean(BooleanOp::Difference, &[base.clone(), far]);
    assert_manifold("disjoint_subtract", &result);
    let (lo, hi) = result.bounds().unwrap();
    assert!((hi.x - lo.x - 20.0).abs() < 1e-9, "disjoint subtract changed the base");

    // Fully contained tool: hollows the base out, leaving two nested shells.
    let inner = primitives::box_mesh(10.0, 10.0, 10.0);
    let result = evaluate_boolean(BooleanOp::Difference, &[base, inner]);
    assert_manifold("contained_subtract", &result);
}

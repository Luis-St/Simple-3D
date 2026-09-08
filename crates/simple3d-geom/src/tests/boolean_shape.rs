//! What a boolean leaves behind: where the hole is, and whether nesting
//! them changes the answer.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn base_anchor_moves_origin_not_shape() {
    let mut m = primitives::box_mesh(40.0, 20.0, 4.0);
    let (lo, hi) = m.bounds().unwrap();
    let size_before = hi - lo;
    m.apply_base_anchor();
    let (lo2, hi2) = m.bounds().unwrap();
    let size_after = hi2 - lo2;
    assert!((size_before.x - size_after.x).abs() < 1e-9);
    assert!((size_before.y - size_after.y).abs() < 1e-9);
    assert!((size_before.z - size_after.z).abs() < 1e-9);
    assert!(lo2.z.abs() < 1e-9, "base anchor should put min Z at 0, got {}", lo2.z);
}

#[test]
pub(crate) fn hole_in_plate_is_round_and_in_the_right_place() {
    // Criterion 4: a 6mm hole through a 40x20x4 plate, 12mm from the left edge.
    let plate = primitives::box_mesh(40.0, 20.0, 4.0);
    let centre_x = -20.0 + 12.0;
    let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 64).translated(Vec3::new(centre_x, 0.0, 0.0));
    let result = evaluate_boolean(BooleanOp::Difference, &[plate, hole]);
    assert_manifold("plate_hole_placement", &result);
    assert_bounds("plate_hole_placement", &result, Vec3::new(40.0, 20.0, 4.0), 1e-9);

    // Every vertex introduced by the cut sits on the hole's circle.
    let on_hole: Vec<&Vec3> =
        result.positions.iter().filter(|p| ((p.x - centre_x).powi(2) + p.y * p.y).sqrt() < 3.0 + 1e-6).collect();
    assert!(!on_hole.is_empty(), "no vertices found on the hole boundary");
    for p in on_hole {
        let r = ((p.x - centre_x).powi(2) + p.y * p.y).sqrt();
        assert!((r - 3.0).abs() < 1e-6, "hole vertex at radius {r}, expected 3.0");
        assert!(p.z.abs() <= 2.0 + 1e-9);
    }
}

#[test]
pub(crate) fn nested_booleans_stay_manifold() {
    // The output of one boolean is the input of the next; a T-junction left
    // behind by the first would compound.
    let plate = primitives::box_mesh(40.0, 20.0, 4.0);
    let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 24).translated(Vec3::new(-8.0, 0.0, 0.0));
    let drilled = evaluate_boolean(BooleanOp::Difference, &[plate, hole]);
    let boss = primitives::cylinder_mesh(8.0, 8.0, 10.0, 24).translated(Vec3::new(12.0, 0.0, 0.0));
    let with_boss = evaluate_boolean(BooleanOp::Union, &[drilled, boss]);
    assert_manifold("nested_union_after_difference", &with_boss);
    let slot = primitives::box_mesh(6.0, 30.0, 6.0);
    let final_mesh = evaluate_boolean(BooleanOp::Difference, &[with_boss, slot]);
    assert_manifold("nested_difference_after_union", &final_mesh);
}

#[test]
pub(crate) fn boolean_evaluation_is_deterministic() {
    // Spec section 5.2: the same tree must always produce the same mesh.
    let build = || {
        let plate = primitives::box_mesh(40.0, 20.0, 4.0);
        let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 32).translated(Vec3::new(-8.0, 0.0, 0.0));
        evaluate_boolean(BooleanOp::Difference, &[plate, hole])
    };
    let a = build();
    let b = build();
    assert_eq!(a.indices, b.indices);
    assert_eq!(a.positions.len(), b.positions.len());
    for (p, q) in a.positions.iter().zip(b.positions.iter()) {
        assert_eq!(p, q);
    }
}

#[test]
pub(crate) fn a_swept_solid_is_closed_by_its_cut_faces() {
    // A shape short of a full turn is a pie slice, and the two faces it was
    // cut on have to close it -- an open sector would export as a shell.
    for sweep in [30.0, 90.0, 180.0, 270.0, 359.0] {
        let name = format!("sweep {sweep}");
        assert_manifold(&format!("cylinder {name}"), &primitives::cylinder_sector_mesh(20.0, 20.0, 10.0, 32, sweep));
        assert_manifold(&format!("ellipse {name}"), &primitives::cylinder_sector_mesh(20.0, 12.0, 10.0, 32, sweep));
        assert_manifold(&format!("cone {name}"), &primitives::cone_sector_mesh(20.0, 0.0, 10.0, 32, sweep));
        assert_manifold(&format!("frustum {name}"), &primitives::cone_sector_mesh(20.0, 8.0, 10.0, 32, sweep));
        assert_manifold(&format!("tube {name}"), &primitives::tube_sector_mesh(20.0, 12.0, 10.0, 32, sweep));
        assert_manifold(&format!("ring {name}"), &primitives::ring_sector_mesh(20.0, 12.0, 2.0, 32, sweep));
    }
}

#[test]
pub(crate) fn a_full_sweep_is_the_unswept_shape_exactly() {
    // The sweep parameter defaults to a full turn, so every existing project
    // has to keep the geometry it had: same vertices, not merely same size.
    let full = primitives::cylinder_sector_mesh(20.0, 12.0, 10.0, 32, 360.0);
    let plain = primitives::cylinder_mesh(20.0, 12.0, 10.0, 32);
    assert_eq!(full.positions, plain.positions);
    assert_eq!(full.indices, plain.indices);
}

#[test]
pub(crate) fn a_quarter_cylinder_spans_one_radius_not_one_diameter() {
    // What makes the X and Y resize handles withdraw on a partial sweep: the
    // shape's width stops being its diameter.
    let quarter = primitives::cylinder_sector_mesh(20.0, 20.0, 10.0, 32, 90.0);
    assert_bounds("quarter cylinder", &quarter, Vec3::new(10.0, 10.0, 10.0), 1e-9);
}

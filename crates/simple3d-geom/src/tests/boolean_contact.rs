//! Bodies meeting at an angle, off-centre, or on a shared plane.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn a_convex_operand_cuts_only_where_its_surface_is() {
    // A convex body's BSP chain cut polygons by every face plane, adding invented edges (1,210 and
    // 2,434 triangles at 64 and 128 segments, versus 816 and 1,584 needed). The count is the measure,
    // since the extra triangles are the bug.
    let crossing = |segments: u32| {
        let a = primitives::cylinder_mesh(20.0, 20.0, 40.0, segments);
        let b = primitives::cylinder_mesh(20.0, 20.0, 40.0, segments)
            .transformed(Vec3::new(6.0, 0.0, 0.0), Vec3::new(0.0, 90.0, 0.0));
        evaluate_boolean(BooleanOp::Union, &[a, b])
    };
    let coarse = crossing(64);
    let fine = crossing(128);
    assert_manifold("two cylinders at 64 segments", &coarse);
    assert_manifold("two cylinders at 128 segments", &fine);
    assert!(coarse.triangle_count() < 1000, "{} triangles at 64 segments, 1210 before", coarse.triangle_count());
    assert!(fine.triangle_count() < 2000, "{} triangles at 128 segments, 2434 before", fine.triangle_count());

    // Doubling the segments doubles the surface, not more; it was quadratic.
    assert!(
        fine.triangle_count() < coarse.triangle_count() * 3,
        "{} triangles against {}",
        fine.triangle_count(),
        coarse.triangle_count()
    );
}

#[test]
pub(crate) fn a_cap_meeting_a_plate_off_axis_is_closed() {
    // Issue A: clipping the plate against the cap's whole tangent-plane chain left slivers within
    // epsilon of the next plane, so an untrimmed corner left a triangle owned by neither body, which
    // `repair::heal` cannot close. Both came back open before the clip was restricted.
    let cap =
        primitives::spherical_cap_mesh(20.0, 6.0, 128).transformed(Vec3::new(3.0, 2.0, 1.0), Vec3::new(10.0, 0.0, 0.0));
    let plate = primitives::plate_mesh(40.0, 40.0, 4.0);
    assert_manifold("plate union an off-axis cap", &evaluate_boolean(BooleanOp::Union, &[plate.clone(), cap.clone()]));
    assert_manifold("plate less an off-axis cap", &evaluate_boolean(BooleanOp::Difference, &[plate, cap]));
}

#[test]
pub(crate) fn an_inverted_convex_clipper_keeps_the_right_side() {
    // Inverted operands must keep the other side with the same planes: the difference and the
    // intersection of a half-sunk sphere must add up to the plate.
    let plate = primitives::box_mesh(40.0, 40.0, 4.0);
    let ball = primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 96).translated(Vec3::new(0.0, 0.0, 2.0));
    let cut = evaluate_boolean(BooleanOp::Difference, &[plate.clone(), ball.clone()]);
    let kept = evaluate_boolean(BooleanOp::Intersection, &[plate.clone(), ball]);
    assert_manifold("a plate less a ball", &cut);
    assert_manifold("a plate meeting a ball", &kept);

    let (whole, a, b) = (volume(&plate), volume(&cut), volume(&kept));
    assert!((a + b - whole).abs() / whole < 1e-6, "{a} + {b} is not {whole}");
    // A real bite: neither empty nor the whole ball.
    assert!(b > whole * 0.05 && b < whole * 0.95, "the bite is {b} of {whole}");
}

/// A plate minus a box sharing two of its side planes, through the general near-face clip. A
/// plate piece in the box's side plane but off its end must be decided by position, not by the
/// coincident-face rule.
#[test]
pub(crate) fn a_difference_sharing_side_planes_is_a_closed_solid() {
    let plate = primitives::plate_mesh(40.0, 20.0, 4.0);
    let cutter = primitives::box_mesh(20.0, 20.0, 20.0);
    let cut = evaluate_boolean(BooleanOp::Difference, &[plate, cutter]);
    assert_manifold("plate minus box", &cut);
    let (lo, hi) = cut.bounds().unwrap();
    assert!(
        (hi.z - 2.0).abs() < 1e-9 && (lo.z + 2.0).abs() < 1e-9,
        "the cut reaches {lo:?}..{hi:?}, past the plate it was taken out of"
    );
}

/// Two round bodies meeting off-centre and off-axis, leaving a sliver owned by neither operand;
/// `repair::cap_boundary_loops` closes the hole.
#[test]
pub(crate) fn round_bodies_meeting_at_an_angle_come_out_closed() {
    let sphere = primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 128);
    let cap =
        primitives::spherical_cap_mesh(20.0, 6.0, 128).transformed(Vec3::new(3.0, 2.0, 1.0), Vec3::new(10.0, 0.0, 0.0));
    for op in [BooleanOp::Union, BooleanOp::Difference, BooleanOp::Intersection] {
        let result = evaluate_boolean(op, &[sphere.clone(), cap.clone()]);
        assert_manifold(&format!("sphere {op:?} cap"), &result);
    }
}

/// The closedness check must refuse holes but accept touching bodies, like four split cells
/// sharing one line, which the old "exactly once each way" rule refused.
#[test]
pub(crate) fn two_touching_bodies_are_closed_and_a_hole_is_not() {
    let mut apart = primitives::box_mesh(20.0, 20.0, 20.0);
    let mut beside = primitives::box_mesh(20.0, 20.0, 20.0);
    for p in &mut beside.positions {
        p.x += 20.0;
    }
    apart.append(&beside);
    assert!(apart.manifold_issue().is_none(), "two bodies meeting face to face: {:?}", apart.manifold_issue());

    // Four cells around one line, as every grid makes.
    let mut grid = Mesh::new();
    for (x, y) in [(0.0, 0.0), (20.0, 0.0), (0.0, 20.0), (20.0, 20.0)] {
        let mut cell = primitives::box_mesh(20.0, 20.0, 20.0);
        for p in &mut cell.positions {
            p.x += x;
            p.y += y;
        }
        grid.append(&cell);
    }
    assert!(grid.manifold_issue().is_none(), "four cells around one line: {:?}", grid.manifold_issue());

    // A body with a face removed is still refused.
    let mut holed = primitives::box_mesh(20.0, 20.0, 20.0);
    holed.indices.pop();
    holed.tags.pop();
    assert!(holed.manifold_issue().is_some(), "a mesh with a hole in it passed");
}

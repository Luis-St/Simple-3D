//! Bodies meeting at an angle, off-centre, or on a shared plane.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn a_convex_operand_cuts_only_where_its_surface_is() {
    // A convex body's BSP is a chain one node per face plane, and clipping a
    // polygon down it cut that polygon by every one of those planes -- planes
    // that are infinite, while the faces they belong to are millimetres away
    // from where the cut lands. Two crossing cylinders came back carrying every
    // one of those invented edges: 1,210 triangles at 64 segments and 2,434 at
    // 128, for a surface that needs 816 and 1,584.
    //
    // The count is the measure here because the extra triangles *are* the bug.
    // Restricting the cut to the faces whose boxes come near the polygon is
    // exact -- the other body's surface can only cross the polygon inside one
    // of its faces -- so what is left is the surface without the invention.
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

    // Doubling the segment count doubles the surface, not more: the cost and
    // the size were quadratic in it while every plane cut every fragment.
    assert!(
        fine.triangle_count() < coarse.triangle_count() * 3,
        "{} triangles against {}",
        fine.triangle_count(),
        coarse.triangle_count()
    );
}

#[test]
pub(crate) fn a_cap_meeting_a_plate_off_axis_is_closed() {
    // Issue A, on the cheapest case that shows it. Clipping the plate against
    // the cap's whole chain of tangent planes shredded the plate's faces into
    // hundreds of slivers around where the cap cuts through, and the corners of
    // those slivers land close enough to the next tangent plane for the fixed
    // epsilon to call them coplanar with it -- so a plane that should have
    // trimmed a corner leaves it standing, a few microns outside the cap, and
    // the triangle between that corner and the cap's own edge belongs to
    // neither body. `repair::heal` cannot close it: there is no T-junction
    // there, only a triangle nothing ever emitted.
    //
    // Both of these came back open before the clip was restricted to the faces
    // the cap's surface actually occupies.
    let cap =
        primitives::spherical_cap_mesh(20.0, 6.0, 128).transformed(Vec3::new(3.0, 2.0, 1.0), Vec3::new(10.0, 0.0, 0.0));
    let plate = primitives::plate_mesh(40.0, 40.0, 4.0);
    assert_manifold("plate union an off-axis cap", &evaluate_boolean(BooleanOp::Union, &[plate.clone(), cap.clone()]));
    assert_manifold("plate less an off-axis cap", &evaluate_boolean(BooleanOp::Difference, &[plate, cap]));
}

#[test]
pub(crate) fn an_inverted_convex_clipper_keeps_the_right_side() {
    // `subtract` and `intersect` invert an operand before clipping with it, so
    // the convex path has to answer the opposite question with the same planes:
    // behind every face is what survives rather than what goes. A sphere sunk
    // halfway into a plate takes a hemisphere's worth of material out of it and
    // leaves that same hemisphere behind as the intersection, so the two must
    // add back up to the plate.
    let plate = primitives::box_mesh(40.0, 40.0, 4.0);
    let ball = primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 96).translated(Vec3::new(0.0, 0.0, 2.0));
    let cut = evaluate_boolean(BooleanOp::Difference, &[plate.clone(), ball.clone()]);
    let kept = evaluate_boolean(BooleanOp::Intersection, &[plate.clone(), ball]);
    assert_manifold("a plate less a ball", &cut);
    assert_manifold("a plate meeting a ball", &kept);

    let (whole, a, b) = (volume(&plate), volume(&cut), volume(&kept));
    assert!((a + b - whole).abs() / whole < 1e-6, "{a} + {b} is not {whole}");
    // The half of the ball below the plate's top face, less nothing: a real
    // bite, not an empty or a whole one.
    assert!(b > whole * 0.05 && b < whole * 0.95, "the bite is {b} of {whole}");
}

/// A plate with a box taken out of it, sharing two of its side planes.
///
/// Small enough that neither operand reaches the convex shortcut, so both go
/// through the general near-face clip -- and the shared planes are what that
/// clip has to get right: a piece of the plate lying *in* the plane of the
/// box's side but off the end of it says nothing about the box at all, and
/// deciding it by the coincident-face rule instead of by where it is turned
/// the difference back into its own operands.
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

/// Two round bodies meeting off-centre and off-axis, which is where the
/// kernel's fixed-epsilon classification leaves a sliver belonging to neither
/// operand. `repair::cap_boundary_loops` is what closes the hole that leaves;
/// without it this is a surface with a triangle missing out of it.
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

/// What the closedness check is for, and what it must not refuse.
///
/// A mesh here is not always one surface: a scene holds several bodies, and two
/// of them may touch. Four cells of a split meet along one line, so that line's
/// welded edge is used four times each way -- and the rule used to be "exactly
/// once each way", which called a perfectly ordinary split broken. What the
/// check is really for is a surface with a hole in it, and that still shows up
/// as a count that does not balance.
#[test]
pub(crate) fn two_touching_bodies_are_closed_and_a_hole_is_not() {
    let mut apart = primitives::box_mesh(20.0, 20.0, 20.0);
    let mut beside = primitives::box_mesh(20.0, 20.0, 20.0);
    for p in &mut beside.positions {
        p.x += 20.0;
    }
    apart.append(&beside);
    assert!(apart.manifold_issue().is_none(), "two bodies meeting face to face: {:?}", apart.manifold_issue());

    // Four of them around one line, which is the arrangement every grid of
    // cells makes and the one the old rule refused.
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

    // And a body with a face taken out of it is still refused.
    let mut holed = primitives::box_mesh(20.0, 20.0, 20.0);
    holed.indices.pop();
    holed.tags.pop();
    assert!(holed.manifold_issue().is_some(), "a mesh with a hole in it passed");
}

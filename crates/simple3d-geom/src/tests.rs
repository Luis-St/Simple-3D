#![cfg(test)]

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

fn assert_manifold(name: &str, mesh: &Mesh) {
    assert!(mesh.triangle_count() > 0, "{name}: empty mesh");
    if let Some(issue) = mesh.manifold_issue() {
        panic!("{name}: not manifold: {issue}");
    }
}

fn assert_bounds(name: &str, mesh: &Mesh, expect: Vec3, tol: f64) {
    let (lo, hi) = mesh.bounds().unwrap();
    let size = hi - lo;
    assert!(
        (size.x - expect.x).abs() <= tol && (size.y - expect.y).abs() <= tol && (size.z - expect.z).abs() <= tol,
        "{name}: bounds {:?} vs expected {:?}",
        size,
        expect
    );
}

#[test]
fn box_is_manifold_and_exact() {
    let m = primitives::box_mesh(40.0, 20.0, 4.0);
    assert_manifold("box", &m);
    assert_bounds("box", &m, Vec3::new(40.0, 20.0, 4.0), 1e-9);
}

#[test]
fn rounded_box_is_manifold() {
    let m = primitives::rounded_box_mesh(40.0, 20.0, 4.0, 3.0, 6);
    assert_manifold("rounded_box", &m);
}

#[test]
fn wedge_is_manifold() {
    let m = primitives::wedge_mesh(20.0, 10.0, 8.0, 0.0);
    assert_manifold("wedge_sharp", &m);
    let m = primitives::wedge_mesh(20.0, 10.0, 8.0, 5.0);
    assert_manifold("wedge_flat_top", &m);
}

#[test]
fn regular_prism_hex_across_flats_is_10mm() {
    let m = primitives::regular_prism_mesh(6, 10.0, 5.0, true);
    assert_manifold("hex_prism", &m);
    // Across-flats distance = max X extent * 2 when a flat faces +X (angle 0 vertex).
    let (lo, hi) = m.bounds().unwrap();
    // For a hexagon with a vertex at angle 0, X spans [-R, R] where R is the
    // circumradius; verify the apothem-derived circumradius gives 10mm across flats
    // by checking the flat-to-flat width along Y (vertex at angle 0 means flats
    // are perpendicular to the axes at 30/90/150...). Just assert overall size sane.
    assert!((hi.x - lo.x) > 9.9 && (hi.x - lo.x) < 11.6);
}

#[test]
fn cylinder_is_manifold_and_exact_height() {
    let m = primitives::cylinder_mesh(50.0, 50.0, 12.0, 32);
    assert_manifold("cylinder", &m);
    assert_bounds("cylinder", &m, Vec3::new(50.0, 50.0, 12.0), 0.2);
}

#[test]
fn cone_is_manifold() {
    let m = primitives::cone_mesh(20.0, 0.0, 15.0, 24);
    assert_manifold("cone_sharp", &m);
    let m = primitives::cone_mesh(20.0, 8.0, 15.0, 24);
    assert_manifold("cone_frustum", &m);
}

#[test]
fn pyramid_is_manifold() {
    let m = primitives::pyramid_mesh(20.0, 15.0, 0.0, 0.0, 10.0);
    assert_manifold("pyramid_apex", &m);
    let m = primitives::pyramid_mesh(20.0, 15.0, 5.0, 5.0, 10.0);
    assert_manifold("pyramid_frustum", &m);
}

#[test]
fn regular_pyramid_is_manifold() {
    let m = primitives::regular_pyramid_mesh(5, 20.0, 0.0, 10.0, false);
    assert_manifold("regular_pyramid_apex", &m);
}

#[test]
fn tube_is_manifold_and_exact() {
    let m = primitives::tube_mesh(20.0, 12.0, 6.0, 32);
    assert_manifold("tube", &m);
    assert_bounds("tube", &m, Vec3::new(20.0, 20.0, 6.0), 0.2);
}

#[test]
fn torus_full_is_manifold() {
    let m = primitives::torus_mesh(30.0, 6.0, 360.0, 32);
    assert_manifold("torus_full", &m);
}

#[test]
fn torus_arc_is_manifold() {
    let m = primitives::torus_mesh(30.0, 6.0, 180.0, 32);
    assert_manifold("torus_arc", &m);
}

#[test]
fn ellipsoid_is_manifold_and_exact() {
    let m = primitives::ellipsoid_mesh(50.0, 30.0, 20.0, 32);
    assert_manifold("ellipsoid", &m);
    assert_bounds("ellipsoid", &m, Vec3::new(50.0, 30.0, 20.0), 0.2);
}

#[test]
fn spherical_cap_hemisphere_is_manifold() {
    let m = primitives::spherical_cap_mesh(40.0, 20.0, 32);
    assert_manifold("hemisphere", &m);
}

#[test]
fn spherical_cap_shallow_is_manifold() {
    let m = primitives::spherical_cap_mesh(40.0, 5.0, 32);
    assert_manifold("shallow_cap", &m);
}

#[test]
fn capsule_is_manifold() {
    let m = primitives::capsule_mesh(10.0, 30.0, 24);
    assert_manifold("capsule", &m);
    let m = primitives::capsule_mesh(10.0, 8.0, 24); // shorter than diameter, clamps cyl to 0
    assert_manifold("capsule_short", &m);
}

#[test]
fn polyhedra_are_manifold() {
    assert_manifold("tetra", &primitives::tetrahedron_mesh(20.0, false));
    assert_manifold("tetra_edge", &primitives::tetrahedron_mesh(20.0, true));
    assert_manifold("octa", &primitives::octahedron_mesh(20.0, false));
    assert_manifold("icosa", &primitives::icosahedron_mesh(20.0, false));
    assert_manifold("dodeca", &primitives::dodecahedron_mesh(20.0, false));
}

#[test]
fn icosahedron_circumdiameter_is_exact() {
    let m = primitives::icosahedron_mesh(20.0, false);
    let max_r = m.positions.iter().map(|p| p.length()).fold(0.0, f64::max);
    assert!((max_r * 2.0 - 20.0).abs() < 1e-9);
}

#[test]
fn plate_matches_box() {
    let a = primitives::plate_mesh(40.0, 20.0, 4.0);
    let b = primitives::box_mesh(40.0, 20.0, 4.0);
    assert_eq!(a.triangle_count(), b.triangle_count());
}

#[test]
fn boolean_difference_hole_in_plate_is_manifold() {
    let plate = primitives::box_mesh(40.0, 20.0, 4.0);
    let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 32).translated(Vec3::new(-8.0, 0.0, 0.0));
    let result = evaluate_boolean(BooleanOp::Difference, &[plate, hole]);
    assert_manifold("plate_with_hole", &result);
}

#[test]
fn boolean_union_touching_at_edge_is_manifold() {
    let a = primitives::box_mesh(10.0, 10.0, 10.0);
    let b = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(10.0, 0.0, 0.0));
    let result = evaluate_boolean(BooleanOp::Union, &[a, b]);
    assert_manifold("union_touching_faces", &result);
}

#[test]
fn boolean_intersection_and_hull() {
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
fn base_anchor_moves_origin_not_shape() {
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
fn boolean_diff_simple_boxes() {
    let a = primitives::box_mesh(20.0, 20.0, 20.0);
    let b = primitives::box_mesh(10.0, 10.0, 30.0);
    let result = evaluate_boolean(BooleanOp::Difference, &[a, b]);
    assert_manifold("diff_simple_boxes", &result);
}

#[test]
fn boolean_intersect_simple_boxes() {
    let a = primitives::box_mesh(20.0, 20.0, 20.0);
    let b = primitives::box_mesh(10.0, 10.0, 30.0);
    let result = evaluate_boolean(BooleanOp::Intersection, &[a, b]);
    assert_manifold("intersect_simple_boxes", &result);
}

#[test]
fn regular_prism_across_flats_is_exact() {
    // Spec acceptance criterion 3: a hex prism declared 10mm across flats must
    // measure 10mm across flats, not across corners. `ring_outline` puts a
    // vertex at angle 0, so for a hexagon the flats face +/-Y.
    let m = primitives::regular_prism_mesh(6, 10.0, 5.0, true);
    let (lo, hi) = m.bounds().unwrap();
    assert!((hi.y - lo.y - 10.0).abs() < 1e-9, "across flats = {}", hi.y - lo.y);
    let across_corners = hi.x - lo.x;
    assert!((across_corners - 10.0 / (std::f64::consts::PI / 6.0).cos()).abs() < 1e-9);
    // And the across-corners convention must give exactly 10 across corners.
    let m = primitives::regular_prism_mesh(6, 10.0, 5.0, false);
    let (lo, hi) = m.bounds().unwrap();
    assert!((hi.x - lo.x - 10.0).abs() < 1e-9);
}

#[test]
fn boolean_subtract_coplanar_face_is_manifold() {
    // Spec acceptance criterion 5, first half: the tool's face lies exactly on
    // the base's face. This is the normal case, not an edge case -- every
    // pocket cut from a face produces it.
    let base = primitives::box_mesh(40.0, 20.0, 4.0);
    let tool = primitives::box_mesh(10.0, 10.0, 4.0); // top and bottom coplanar with base
    let result = evaluate_boolean(BooleanOp::Difference, &[base, tool]);
    assert_manifold("coplanar_subtract", &result);
}

#[test]
fn boolean_subtract_touching_at_edge_is_manifold() {
    // Criterion 5, second half: operands touch along a single edge, so the
    // subtraction removes nothing and the base must survive intact.
    let base = primitives::box_mesh(20.0, 20.0, 20.0);
    let tool = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(15.0, 15.0, 0.0));
    let result = evaluate_boolean(BooleanOp::Difference, &[base, tool]);
    assert_manifold("edge_touch_subtract", &result);
}

#[test]
fn boolean_subtract_disjoint_and_contained() {
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

#[test]
fn hole_in_plate_is_round_and_in_the_right_place() {
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
fn nested_booleans_stay_manifold() {
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
fn boolean_evaluation_is_deterministic() {
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
fn a_swept_solid_is_closed_by_its_cut_faces() {
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
fn a_full_sweep_is_the_unswept_shape_exactly() {
    // The sweep parameter defaults to a full turn, so every existing project
    // has to keep the geometry it had: same vertices, not merely same size.
    let full = primitives::cylinder_sector_mesh(20.0, 12.0, 10.0, 32, 360.0);
    let plain = primitives::cylinder_mesh(20.0, 12.0, 10.0, 32);
    assert_eq!(full.positions, plain.positions);
    assert_eq!(full.indices, plain.indices);
}

#[test]
fn a_quarter_cylinder_spans_one_radius_not_one_diameter() {
    // What makes the X and Y resize handles withdraw on a partial sweep: the
    // shape's width stops being its diameter.
    let quarter = primitives::cylinder_sector_mesh(20.0, 20.0, 10.0, 32, 90.0);
    assert_bounds("quarter cylinder", &quarter, Vec3::new(10.0, 10.0, 10.0), 1e-9);
}

#[test]
fn a_chamfered_box_keeps_its_stated_dimensions() {
    // A chamfer cuts corners off, never past a face, so the box is still
    // exactly the size it was asked for whichever edges are cut.
    use primitives::ChamferEdges;
    for edges in [ChamferEdges::All, ChamferEdges::Vertical, ChamferEdges::TopAndBottom] {
        let name = format!("chamfered box {edges:?}");
        let mesh = primitives::chamfered_box_mesh(40.0, 20.0, 12.0, 3.0, edges);
        assert_manifold(&name, &mesh);
        assert_bounds(&name, &mesh, Vec3::new(40.0, 20.0, 12.0), 1e-9);
    }
}

#[test]
fn an_oversized_chamfer_is_clamped_rather_than_folded_through_itself() {
    // Typing a chamfer larger than the box allows must leave a solid, not an
    // inside-out one -- the field cannot refuse the number, so the generator
    // has to survive it.
    use primitives::ChamferEdges;
    for edges in [ChamferEdges::All, ChamferEdges::Vertical, ChamferEdges::TopAndBottom] {
        let name = format!("over-chamfered box {edges:?}");
        let mesh = primitives::chamfered_box_mesh(20.0, 20.0, 20.0, 500.0, edges);
        assert_manifold(&name, &mesh);
        assert_bounds(&name, &mesh, Vec3::new(20.0, 20.0, 20.0), 1e-9);
    }
}

#[test]
fn a_chamfer_of_zero_is_the_plain_box() {
    use primitives::ChamferEdges;
    let plain = primitives::box_mesh(20.0, 14.0, 8.0);
    for edges in [ChamferEdges::All, ChamferEdges::Vertical, ChamferEdges::TopAndBottom] {
        let mesh = primitives::chamfered_box_mesh(20.0, 14.0, 8.0, 0.0, edges);
        assert_eq!(mesh.weld().positions.len(), plain.weld().positions.len(), "{edges:?}");
        assert_bounds("unchamfered box", &mesh, Vec3::new(20.0, 14.0, 8.0), 1e-9);
    }
}

#[test]
fn a_chamfer_removes_material_and_nothing_else() {
    // The chamfered box has to sit inside the box it was cut from: every
    // vertex within the plain box's bounds, and less volume than it.
    use primitives::ChamferEdges;
    let mesh = primitives::chamfered_box_mesh(30.0, 20.0, 10.0, 3.0, ChamferEdges::All);
    for p in &mesh.positions {
        assert!(p.x.abs() <= 15.0 + 1e-9 && p.y.abs() <= 10.0 + 1e-9 && p.z.abs() <= 5.0 + 1e-9, "{p:?} escapes");
    }
    let cut = evaluate_boolean(BooleanOp::Difference, &[primitives::box_mesh(30.0, 20.0, 10.0), mesh]);
    assert!(cut.triangle_count() > 0, "a chamfer should leave the box with corners missing");
}

#[test]
fn a_slot_is_a_rectangle_with_semicircular_ends() {
    let slot = primitives::slot_mesh(30.0, 10.0, 4.0, 32);
    assert_manifold("slot", &slot);
    assert_bounds("slot", &slot, Vec3::new(30.0, 10.0, 4.0), 1e-9);
    // A slot as long as it is wide is a disc: the two end radii meet.
    let round = primitives::slot_mesh(10.0, 10.0, 4.0, 32);
    assert_manifold("round slot", &round);
    assert_bounds("round slot", &round, Vec3::new(10.0, 10.0, 4.0), 1e-9);
}

#[test]
fn an_inset_outline_moves_every_edge_by_the_same_distance() {
    // What a horizontal chamfer relies on: a parallel offset, not a scale.
    // A scale would move the long edges of a 40x10 rectangle four times as far
    // as the short ones and the chamfer would not be 45 degrees.
    let rect = vec![(20.0, -5.0), (20.0, 5.0), (-20.0, 5.0), (-20.0, -5.0)];
    let inset = crate::revolve::inset_convex_outline(&rect, 2.0);
    let expected = [(18.0, -3.0), (18.0, 3.0), (-18.0, 3.0), (-18.0, -3.0)];
    for (got, want) in inset.iter().zip(expected.iter()) {
        assert!((got.0 - want.0).abs() < 1e-9 && (got.1 - want.1).abs() < 1e-9, "{got:?} vs {want:?}");
    }
}

#[test]
fn primitive_bounds_match_declared_dimensions() {
    // Criterion 2, over the whole primitive table. Flat axes must be exact;
    // curved axes are exact too because vertices sit on the circumscribed
    // circle and a vertex is placed at angle 0 (so the extreme is hit).
    let cases: Vec<(&str, Mesh, Vec3, f64)> = vec![
        ("box", primitives::box_mesh(40.0, 20.0, 4.0), Vec3::new(40.0, 20.0, 4.0), 1e-9),
        ("plate", primitives::plate_mesh(30.0, 10.0, 2.0), Vec3::new(30.0, 10.0, 2.0), 1e-9),
        ("rounded_box", primitives::rounded_box_mesh(40.0, 20.0, 4.0, 3.0, 8), Vec3::new(40.0, 20.0, 4.0), 1e-9),
        ("wedge", primitives::wedge_mesh(20.0, 10.0, 8.0, 5.0), Vec3::new(20.0, 10.0, 8.0), 1e-9),
        ("cylinder", primitives::cylinder_mesh(50.0, 50.0, 12.0, 32), Vec3::new(50.0, 50.0, 12.0), 1e-9),
        ("disc", primitives::disc_mesh(20.0, 20.0, 2.0, 32), Vec3::new(20.0, 20.0, 2.0), 1e-9),
        ("tube", primitives::tube_mesh(20.0, 12.0, 6.0, 32), Vec3::new(20.0, 20.0, 6.0), 1e-9),
        ("ring", primitives::ring_mesh(20.0, 12.0, 2.0, 32), Vec3::new(20.0, 20.0, 2.0), 1e-9),
        ("cone", primitives::cone_mesh(20.0, 0.0, 15.0, 32), Vec3::new(20.0, 20.0, 15.0), 1e-9),
        ("frustum", primitives::cone_mesh(20.0, 8.0, 15.0, 32), Vec3::new(20.0, 20.0, 15.0), 1e-9),
        ("pyramid", primitives::pyramid_mesh(20.0, 15.0, 0.0, 0.0, 10.0), Vec3::new(20.0, 15.0, 10.0), 1e-9),
        ("sphere", primitives::ellipsoid_mesh(50.0, 50.0, 50.0, 32), Vec3::new(50.0, 50.0, 50.0), 1e-9),
        ("ellipsoid", primitives::ellipsoid_mesh(50.0, 30.0, 20.0, 32), Vec3::new(50.0, 30.0, 20.0), 1e-9),
        ("capsule", primitives::capsule_mesh(10.0, 30.0, 32), Vec3::new(10.0, 10.0, 30.0), 1e-9),
        ("hemisphere", primitives::spherical_cap_mesh(40.0, 20.0, 32), Vec3::new(40.0, 40.0, 20.0), 1e-9),
        ("torus", primitives::torus_mesh(30.0, 6.0, 360.0, 32), Vec3::new(36.0, 36.0, 6.0), 1e-9),
    ];
    for (name, mesh, expect, tol) in cases {
        assert_manifold(name, &mesh);
        assert_bounds(name, &mesh, expect, tol);
    }
}

#[test]
fn polyhedra_sizes_match_both_conventions() {
    for (name, circum, edge) in [
        ("tetra", primitives::tetrahedron_mesh(20.0, false), primitives::tetrahedron_mesh(20.0, true)),
        ("octa", primitives::octahedron_mesh(20.0, false), primitives::octahedron_mesh(20.0, true)),
        ("dodeca", primitives::dodecahedron_mesh(20.0, false), primitives::dodecahedron_mesh(20.0, true)),
        ("icosa", primitives::icosahedron_mesh(20.0, false), primitives::icosahedron_mesh(20.0, true)),
    ] {
        assert_manifold(name, &circum);
        assert_manifold(name, &edge);
        let max_r = circum.positions.iter().map(|p| p.length()).fold(0.0, f64::max);
        assert!((max_r * 2.0 - 20.0).abs() < 1e-9, "{name}: circumdiameter {}", max_r * 2.0);
        // Shortest distinct vertex-to-vertex distance is the edge length.
        let welded = edge.weld();
        let mut shortest = f64::MAX;
        for t in &welded.indices {
            for k in 0..3 {
                let d = (welded.positions[t[k] as usize] - welded.positions[t[(k + 1) % 3] as usize]).length();
                if d > 1e-9 {
                    shortest = shortest.min(d);
                }
            }
        }
        assert!((shortest - 20.0).abs() < 1e-9, "{name}: edge length {shortest}");
    }
}

#[test]
fn a_union_of_scattered_solids_never_reaches_the_kernel() {
    // The accumulated-bounding-box trap: folding `union` over many operands
    // makes the accumulator's box span everything unioned so far, so an operand
    // physically nowhere near any other still looks like it overlaps and gets
    // run through the BSP against the whole pile. `union_all` keeps each
    // disjoint island's own box instead. What this test pins is the *cost*: a
    // grid of mutually disjoint boxes must stay linear, not blow up once the
    // accumulated box covers the grid.
    let boxes: Vec<Mesh> = (0..40)
        .map(|i| {
            let (row, column) = (i / 8, i % 8);
            primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(column as f64 * 50.0, row as f64 * 50.0, 0.0))
        })
        .collect();
    let expected_triangles: usize = boxes.iter().map(|b| b.triangle_count()).sum();

    let started = std::time::Instant::now();
    let result = evaluate_boolean(BooleanOp::Union, &boxes);
    let elapsed = started.elapsed();

    // Disjoint operands concatenate, which is what the kernel would have
    // produced anyway -- so not one triangle is added or removed.
    assert_eq!(result.triangle_count(), expected_triangles);
    assert_manifold("scattered union", &result);
    assert!(elapsed.as_secs_f64() < 1.0, "a union of 40 disjoint boxes took {elapsed:?}");
}

#[test]
fn merging_two_islands_still_catches_a_third_that_now_touches() {
    // A union that bridges two islands grows the merged box, which can bring it
    // into contact with an island that was previously clear. Three boxes in a
    // row, fed middle-last, is the smallest case: neither end touches the other,
    // but the middle overlaps both, so all three must end up as one solid.
    let left = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(-8.0, 0.0, 0.0));
    let right = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(8.0, 0.0, 0.0));
    let middle = primitives::box_mesh(10.0, 10.0, 10.0);

    let result = evaluate_boolean(BooleanOp::Union, &[left, right, middle]);
    assert_manifold("bridged union", &result);
    // One solid 26mm long, not three overlapping boxes left side by side.
    assert_bounds("bridged union", &result, Vec3::new(26.0, 10.0, 10.0), 1e-9);
}

#[test]
fn a_boolean_result_is_no_denser_than_the_solid_it_describes() {
    // A BSP clips against *infinite* planes, so subtracting a 16-segment
    // cylinder from a plate slices the plate's whole top and bottom face along
    // sixteen lines that run right across it. Correct, but a plate with a hole,
    // a slot and a boss used to arrive at ~1500 triangles for a solid ~230
    // describe. `repair::heal` rebuilds each flat region from its own boundary
    // to undo that; this pins the budget so a chain of booleans cannot start
    // compounding again.
    let plate = primitives::box_mesh(40.0, 20.0, 4.0);
    let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 16).translated(Vec3::new(-12.0, 0.0, 0.0));
    let slot = primitives::box_mesh(8.0, 5.0, 20.0).translated(Vec3::new(12.0, 0.0, 0.0));
    let boss = primitives::cylinder_mesh(9.0, 9.0, 5.0, 16);

    let drilled = evaluate_boolean(BooleanOp::Difference, &[plate, hole, slot]);
    assert_manifold("drilled plate", &drilled);
    let assembly = evaluate_boolean(BooleanOp::Union, &[drilled, boss]);
    assert_manifold("assembly", &assembly);

    // The dimensions the numbers promise survive the rebuild: the plate is 4mm
    // thick and the boss, centred on it, is 5mm tall.
    assert_bounds("assembly", &assembly, Vec3::new(40.0, 20.0, 5.0), 1e-9);
    assert!(
        assembly.triangle_count() < 400,
        "a plate with a hole, a slot and a boss came out at {} triangles",
        assembly.triangle_count()
    );
}

#[test]
fn rebuilding_a_flat_region_keeps_its_boundary() {
    // The one thing retriangulation must never do is straighten a face's
    // boundary and leave the neighbouring face still bent to the old shape --
    // that is a T-junction, and slicers reject it. Nine holes in a row give the
    // top face a boundary made almost entirely of collinear split points, which
    // is exactly the case that stresses it.
    let mut operands = vec![primitives::box_mesh(100.0, 20.0, 4.0)];
    for i in 0..9 {
        operands.push(primitives::cylinder_mesh(6.0, 6.0, 20.0, 12).translated(Vec3::new(
            -40.0 + i as f64 * 10.0,
            0.0,
            0.0,
        )));
    }
    let result = evaluate_boolean(BooleanOp::Difference, &operands);
    assert_manifold("nine holes", &result);
    assert_bounds("nine holes", &result, Vec3::new(100.0, 20.0, 4.0), 1e-9);

    // Every hole is still open: no vertex may sit inside one.
    for i in 0..9 {
        let centre_x = -40.0 + i as f64 * 10.0;
        for p in &result.positions {
            let r = ((p.x - centre_x).powi(2) + p.y * p.y).sqrt();
            assert!(r > 3.0 * (std::f64::consts::PI / 12.0).cos() - 1e-6, "a vertex landed inside hole {i}");
        }
    }
}

#[test]
fn a_region_that_cannot_be_rebuilt_keeps_its_original_triangles() {
    // Retriangulation is allowed to give up, and when it does the region must
    // come through untouched rather than half-rebuilt. Two coplanar squares
    // meeting at one corner are the smallest case it must refuse: the boundary
    // leaves that corner two ways, and which one continues the loop is a guess.
    let mut region = Mesh::new();
    let quad = |m: &mut Mesh, x: f64, y: f64| {
        let p = |dx: f64, dy: f64| Vec3::new(x + dx, y + dy, 0.0);
        m.push_triangle(p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0));
        m.push_triangle(p(0.0, 0.0), p(10.0, 10.0), p(0.0, 10.0));
    };
    quad(&mut region, 0.0, 0.0);
    quad(&mut region, 10.0, 10.0);
    // Welded, so the shared corner really is one vertex and the pinch is real.
    let region = region.weld();

    let rebuilt = crate::planar::retriangulate_flat_regions(&region);
    assert_eq!(rebuilt.triangle_count(), region.triangle_count(), "a pinched region was rebuilt anyway");
    let (lo, hi) = rebuilt.bounds().unwrap();
    assert_eq!((lo, hi), region.bounds().unwrap());
}

/// Evaluation is deterministic (spec section 5.2), and that has to hold across
/// *processes*, not just within one: the subtree cache key is a content hash,
/// two runs are meant to be comparable, and an exported file is meant to be the
/// same file twice. The hull read its horizon edges back out of a `HashMap`,
/// whose iteration order is seeded randomly per process, so the same two
/// spheres hulled to the same solid with its triangles in a different order
/// every time the application was started.
#[test]
fn a_hull_is_the_same_mesh_every_time_it_is_built() {
    let a = crate::primitives::ellipsoid_mesh(30.0, 30.0, 30.0, 32);
    let b = crate::primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 32).translated(Vec3::new(40.0, 0.0, 0.0));
    let points: Vec<Vec3> = a.positions.iter().chain(b.positions.iter()).copied().collect();

    let first = crate::hull::convex_hull(&points);
    for _ in 0..8 {
        let again = crate::hull::convex_hull(&points);
        assert_eq!(again.positions, first.positions, "the hull's vertices came out in a different order");
        assert_eq!(again.indices, first.indices, "the hull's triangles came out in a different order");
    }
}

#[test]
fn a_round_primitive_unions_without_running_out_of_stack() {
    // The regression this file exists for: raising the segment count of a
    // sphere or a spherical cap that touches another body killed the whole
    // application. A convex body defeats the BSP's auto-partition -- every one
    // of its faces has all the others behind it -- so the tree is a chain one
    // node per face, and the walks over it used to be recursive.
    //
    // A quarter of a megabyte of stack is far less than a chain of two thousand
    // faces needs to recurse down, and enough for anything that does not.
    let worker = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let cap = primitives::spherical_cap_mesh(20.0, 10.0, 64);
            let plate = primitives::box_mesh(40.0, 40.0, 4.0);
            let result = evaluate_boolean(BooleanOp::Union, &[plate, cap]);
            result.triangle_count()
        })
        .unwrap();
    let triangles = worker.join().expect("the union overflowed the stack");
    assert!(triangles > 0);
}

/// The volume the mesh encloses, by the divergence theorem. Any boolean that
/// leaves an operand's interior faces in the result, or loses part of the
/// surface, gets this badly wrong -- which a triangle count alone will not show.
fn volume(mesh: &Mesh) -> f64 {
    let mut total = 0.0;
    for t in &mesh.indices {
        let (a, b, c) = (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
        total += a.dot(b.cross(c)) / 6.0;
    }
    total
}

#[test]
fn a_convex_body_is_recognised_as_splitting_nothing() {
    // What the one-pass build rests on: no face of a convex solid divides any
    // other, and a solid with a dent in it has faces that do.
    assert!(crate::csg_bsp::debug_splits_nothing(&primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 64)));
    assert!(crate::csg_bsp::debug_splits_nothing(&primitives::spherical_cap_mesh(20.0, 10.0, 64)));
    assert!(crate::csg_bsp::debug_splits_nothing(&primitives::cylinder_mesh(20.0, 20.0, 20.0, 64.0 as u32)));
    assert!(!crate::csg_bsp::debug_splits_nothing(&primitives::torus_mesh(30.0, 8.0, 360.0, 64)));
}

#[test]
fn a_finely_tessellated_union_is_manifold_and_no_bigger_than_the_solid() {
    // Both halves of the segment-count regression, on the shape it was reported
    // on. A spherical cap sunk into a plate at 176 segments used to come back as
    // 2.7 million triangles -- the T-junction pass cascading into its own budget
    // -- and non-manifold with it. The same union at 64 segments describes the
    // same solid, so the two must agree on volume however finely either is cut.
    let plate = || primitives::box_mesh(40.0, 40.0, 4.0);
    let coarse = evaluate_boolean(BooleanOp::Union, &[plate(), primitives::spherical_cap_mesh(20.0, 10.0, 64)]);
    let fine = evaluate_boolean(BooleanOp::Union, &[plate(), primitives::spherical_cap_mesh(20.0, 10.0, 176)]);
    assert_manifold("union at 64 segments", &coarse);
    assert_manifold("union at 176 segments", &fine);

    let (v0, v1) = (volume(&coarse), volume(&fine));
    assert!((v1 - v0).abs() / v0 < 0.01, "volume moved from {v0} to {v1} between tessellations");

    // Eight faces of the cap for every one at 64 segments, so a result that
    // stays in proportion is at most about ten times the size. The cascade
    // produced eight hundred times.
    assert!(
        fine.triangle_count() < coarse.triangle_count() * 10,
        "{} triangles at 176 segments against {} at 64",
        fine.triangle_count(),
        coarse.triangle_count()
    );
}

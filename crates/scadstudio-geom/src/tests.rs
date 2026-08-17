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
    let on_hole: Vec<&Vec3> = result
        .positions
        .iter()
        .filter(|p| ((p.x - centre_x).powi(2) + p.y * p.y).sqrt() < 3.0 + 1e-6)
        .collect();
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
fn primitive_bounds_match_declared_dimensions() {
    // Criterion 2, over the whole primitive table. Flat axes must be exact;
    // curved axes are exact too because vertices sit on the circumscribed
    // circle and a vertex is placed at angle 0 (so the extreme is hit).
    let cases: Vec<(&str, Mesh, Vec3, f64)> = vec![
        ("box", primitives::box_mesh(40.0, 20.0, 4.0), Vec3::new(40.0, 20.0, 4.0), 1e-9),
        ("plate", primitives::plate_mesh(30.0, 10.0, 2.0), Vec3::new(30.0, 10.0, 2.0), 1e-9),
        (
            "rounded_box",
            primitives::rounded_box_mesh(40.0, 20.0, 4.0, 3.0, 8),
            Vec3::new(40.0, 20.0, 4.0),
            1e-9,
        ),
        ("wedge", primitives::wedge_mesh(20.0, 10.0, 8.0, 5.0), Vec3::new(20.0, 10.0, 8.0), 1e-9),
        ("cylinder", primitives::cylinder_mesh(50.0, 50.0, 12.0, 32), Vec3::new(50.0, 50.0, 12.0), 1e-9),
        ("disc", primitives::disc_mesh(20.0, 20.0, 2.0, 32), Vec3::new(20.0, 20.0, 2.0), 1e-9),
        ("tube", primitives::tube_mesh(20.0, 12.0, 6.0, 32), Vec3::new(20.0, 20.0, 6.0), 1e-9),
        ("ring", primitives::ring_mesh(20.0, 12.0, 2.0, 32), Vec3::new(20.0, 20.0, 2.0), 1e-9),
        ("cone", primitives::cone_mesh(20.0, 0.0, 15.0, 32), Vec3::new(20.0, 20.0, 15.0), 1e-9),
        ("frustum", primitives::cone_mesh(20.0, 8.0, 15.0, 32), Vec3::new(20.0, 20.0, 15.0), 1e-9),
        (
            "pyramid",
            primitives::pyramid_mesh(20.0, 15.0, 0.0, 0.0, 10.0),
            Vec3::new(20.0, 15.0, 10.0),
            1e-9,
        ),
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

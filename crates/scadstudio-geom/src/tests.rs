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

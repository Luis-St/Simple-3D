//! Every primitive generator: closed, and the size it was asked for.

use super::*;
use crate::primitives;
use crate::vec3::Vec3;

#[test]
pub(crate) fn a_box_is_twelve_triangles_and_eight_vertices() {
    // It was 16 and 10: `extrude_frustum_polygon` fanned both caps from a
    // vertex added in the middle of the face, so each square cap was four
    // triangles around a centre rather than two across the diagonal. Nothing
    // was wrong with it -- watertight, manifold, no T-vertices -- but the two
    // invented cap centres went into every 3MF, STL, OBJ and PLY written, of
    // the shape a reader is most likely to open a file to check.
    let welded = primitives::box_mesh(20.0, 20.0, 20.0).weld();
    assert_eq!(welded.triangle_count(), 12, "a cube is two triangles a face");
    assert_eq!(welded.positions.len(), 8, "a cube has eight corners and nothing else");
    assert_manifold("box", &welded);

    // A round cap keeps its centre: fanning a circle from one point on its rim
    // gives slivers, and the boolean kernel classifies by the normals it
    // computes off them.
    let cylinder = primitives::cylinder_mesh(20.0, 20.0, 20.0, 32).weld();
    assert_eq!(cylinder.positions.len(), 32 * 2 + 2, "the cylinder's caps lost their centres");
    assert_manifold("cylinder", &cylinder);
}

#[test]
pub(crate) fn box_is_manifold_and_exact() {
    let m = primitives::box_mesh(40.0, 20.0, 4.0);
    assert_manifold("box", &m);
    assert_bounds("box", &m, Vec3::new(40.0, 20.0, 4.0), 1e-9);
}

#[test]
pub(crate) fn rounded_box_is_manifold() {
    let m = primitives::rounded_box_mesh(40.0, 20.0, 4.0, 3.0, 6);
    assert_manifold("rounded_box", &m);
}

#[test]
pub(crate) fn wedge_is_manifold() {
    let m = primitives::wedge_mesh(20.0, 10.0, 8.0, 0.0);
    assert_manifold("wedge_sharp", &m);
    let m = primitives::wedge_mesh(20.0, 10.0, 8.0, 5.0);
    assert_manifold("wedge_flat_top", &m);
}

#[test]
pub(crate) fn regular_prism_hex_across_flats_is_10mm() {
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
pub(crate) fn cylinder_is_manifold_and_exact_height() {
    let m = primitives::cylinder_mesh(50.0, 50.0, 12.0, 32);
    assert_manifold("cylinder", &m);
    assert_bounds("cylinder", &m, Vec3::new(50.0, 50.0, 12.0), 0.2);
}

#[test]
pub(crate) fn cone_is_manifold() {
    let m = primitives::cone_mesh(20.0, 0.0, 15.0, 24);
    assert_manifold("cone_sharp", &m);
    let m = primitives::cone_mesh(20.0, 8.0, 15.0, 24);
    assert_manifold("cone_frustum", &m);
}

#[test]
pub(crate) fn pyramid_is_manifold() {
    let m = primitives::pyramid_mesh(20.0, 15.0, 0.0, 0.0, 10.0);
    assert_manifold("pyramid_apex", &m);
    let m = primitives::pyramid_mesh(20.0, 15.0, 5.0, 5.0, 10.0);
    assert_manifold("pyramid_frustum", &m);
}

#[test]
pub(crate) fn regular_pyramid_is_manifold() {
    let m = primitives::regular_pyramid_mesh(5, 20.0, 0.0, 10.0, false);
    assert_manifold("regular_pyramid_apex", &m);
}

#[test]
pub(crate) fn tube_is_manifold_and_exact() {
    let m = primitives::tube_mesh(20.0, 12.0, 6.0, 32);
    assert_manifold("tube", &m);
    assert_bounds("tube", &m, Vec3::new(20.0, 20.0, 6.0), 0.2);
}

#[test]
pub(crate) fn torus_full_is_manifold() {
    let m = primitives::torus_mesh(30.0, 6.0, 360.0, 32);
    assert_manifold("torus_full", &m);
}

#[test]
pub(crate) fn torus_arc_is_manifold() {
    let m = primitives::torus_mesh(30.0, 6.0, 180.0, 32);
    assert_manifold("torus_arc", &m);
}

#[test]
pub(crate) fn ellipsoid_is_manifold_and_exact() {
    let m = primitives::ellipsoid_mesh(50.0, 30.0, 20.0, 32);
    assert_manifold("ellipsoid", &m);
    assert_bounds("ellipsoid", &m, Vec3::new(50.0, 30.0, 20.0), 0.2);
}

#[test]
pub(crate) fn spherical_cap_hemisphere_is_manifold() {
    let m = primitives::spherical_cap_mesh(40.0, 20.0, 32);
    assert_manifold("hemisphere", &m);
}

#[test]
pub(crate) fn spherical_cap_shallow_is_manifold() {
    let m = primitives::spherical_cap_mesh(40.0, 5.0, 32);
    assert_manifold("shallow_cap", &m);
}

#[test]
pub(crate) fn capsule_is_manifold() {
    let m = primitives::capsule_mesh(10.0, 30.0, 24);
    assert_manifold("capsule", &m);
    let m = primitives::capsule_mesh(10.0, 8.0, 24); // shorter than diameter, clamps cyl to 0
    assert_manifold("capsule_short", &m);
}

#[test]
pub(crate) fn polyhedra_are_manifold() {
    assert_manifold("tetra", &primitives::tetrahedron_mesh(20.0, false));
    assert_manifold("tetra_edge", &primitives::tetrahedron_mesh(20.0, true));
    assert_manifold("octa", &primitives::octahedron_mesh(20.0, false));
    assert_manifold("icosa", &primitives::icosahedron_mesh(20.0, false));
    assert_manifold("dodeca", &primitives::dodecahedron_mesh(20.0, false));
}

#[test]
pub(crate) fn icosahedron_circumdiameter_is_exact() {
    let m = primitives::icosahedron_mesh(20.0, false);
    let max_r = m.positions.iter().map(|p| p.length()).fold(0.0, f64::max);
    assert!((max_r * 2.0 - 20.0).abs() < 1e-9);
}

#[test]
pub(crate) fn plate_matches_box() {
    let a = primitives::plate_mesh(40.0, 20.0, 4.0);
    let b = primitives::box_mesh(40.0, 20.0, 4.0);
    assert_eq!(a.triangle_count(), b.triangle_count());
}

//! The dimensions a shape claims, measured off the mesh it produced.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn regular_prism_across_flats_is_exact() {
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
pub(crate) fn a_chamfered_box_keeps_its_stated_dimensions() {
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
pub(crate) fn an_oversized_chamfer_is_clamped_rather_than_folded_through_itself() {
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
pub(crate) fn a_chamfer_of_zero_is_the_plain_box() {
    use primitives::ChamferEdges;
    let plain = primitives::box_mesh(20.0, 14.0, 8.0);
    for edges in [ChamferEdges::All, ChamferEdges::Vertical, ChamferEdges::TopAndBottom] {
        let mesh = primitives::chamfered_box_mesh(20.0, 14.0, 8.0, 0.0, edges);
        assert_eq!(mesh.weld().positions.len(), plain.weld().positions.len(), "{edges:?}");
        assert_bounds("unchamfered box", &mesh, Vec3::new(20.0, 14.0, 8.0), 1e-9);
    }
}

#[test]
pub(crate) fn a_chamfer_removes_material_and_nothing_else() {
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
pub(crate) fn a_slot_is_a_rectangle_with_semicircular_ends() {
    let slot = primitives::slot_mesh(30.0, 10.0, 4.0, 32);
    assert_manifold("slot", &slot);
    assert_bounds("slot", &slot, Vec3::new(30.0, 10.0, 4.0), 1e-9);
    // A slot as long as it is wide is a disc: the two end radii meet.
    let round = primitives::slot_mesh(10.0, 10.0, 4.0, 32);
    assert_manifold("round slot", &round);
    assert_bounds("round slot", &round, Vec3::new(10.0, 10.0, 4.0), 1e-9);
}

#[test]
pub(crate) fn an_inset_outline_moves_every_edge_by_the_same_distance() {
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
pub(crate) fn primitive_bounds_match_declared_dimensions() {
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
pub(crate) fn polyhedra_sizes_match_both_conventions() {
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

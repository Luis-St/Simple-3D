//! Rebuilding a flat region from its boundary.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn rebuilding_a_flat_region_keeps_its_boundary() {
    // Retriangulation must never straighten a boundary its neighbour still follows (a T-junction).
    // Nine holes in a row make a boundary of mostly collinear split points.
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

    // Every hole is still open: no vertex inside one.
    for i in 0..9 {
        let centre_x = -40.0 + i as f64 * 10.0;
        for p in &result.positions {
            let r = ((p.x - centre_x).powi(2) + p.y * p.y).sqrt();
            assert!(r > 3.0 * (std::f64::consts::PI / 12.0).cos() - 1e-6, "a vertex landed inside hole {i}");
        }
    }
}

#[test]
pub(crate) fn a_region_that_cannot_be_rebuilt_keeps_its_original_triangles() {
    // A region it cannot rebuild must come through untouched: two squares meeting at one corner,
    // where the boundary's continuation is a guess.
    let mut region = Mesh::new();
    let quad = |m: &mut Mesh, x: f64, y: f64| {
        let p = |dx: f64, dy: f64| Vec3::new(x + dx, y + dy, 0.0);
        m.push_triangle(p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0));
        m.push_triangle(p(0.0, 0.0), p(10.0, 10.0), p(0.0, 10.0));
    };
    quad(&mut region, 0.0, 0.0);
    quad(&mut region, 10.0, 10.0);
    // Welded, so the shared corner is really one vertex.
    let region = region.weld();

    let rebuilt = crate::planar::retriangulate_flat_regions(&region);
    assert_eq!(rebuilt.triangle_count(), region.triangle_count(), "a pinched region was rebuilt anyway");
    let (lo, hi) = rebuilt.bounds().unwrap();
    assert_eq!((lo, hi), region.bounds().unwrap());
}

#[test]
pub(crate) fn copies_meeting_off_centre_leave_no_face_folded_over_the_notch() {
    // Regression: three boxes with the middle one set back put an inner corner in line with two
    // outer ones, and an ear across that corner laid a phantom wedge of floor in the notch.
    let cube = || primitives::box_mesh(20.0, 20.0, 20.0);
    let result = evaluate_boolean(
        BooleanOp::Union,
        &[cube(), cube().translated(Vec3::new(20.0, 10.0, 0.0)), cube().translated(Vec3::new(40.0, 0.0, 0.0))],
    );
    assert_manifold("staggered boxes", &result);
    for t in &result.indices {
        let [a, b, c] = t.map(|i| result.positions[i as usize]);
        if [a, b, c].iter().all(|p| (p.z + 10.0).abs() < 1e-9) {
            assert!((b - a).cross(c - a).z < 0.0, "a bottom triangle {a:?} {b:?} {c:?} faces up");
        }
    }
}

#[test]
pub(crate) fn an_ear_whose_new_side_runs_through_a_corner_is_not_taken() {
    // The same outline alone: folds and giving up both came from ears whose third side crossed a corner.
    let outlines: [&[(f64, f64)]; 2] = [
        &[
            (-10.0, -10.0),
            (10.0, -10.0),
            (10.0, 0.0),
            (30.0, 0.0),
            (30.0, 20.0),
            (10.0, 20.0),
            (10.0, 10.0),
            (-10.0, 10.0),
        ],
        &[
            (-10.0, -10.0),
            (10.0, -10.0),
            (10.0, 0.0),
            (30.0, 0.0),
            (30.0, -10.0),
            (50.0, -10.0),
            (50.0, 10.0),
            (30.0, 10.0),
            (30.0, 20.0),
            (10.0, 20.0),
            (10.0, 10.0),
            (-10.0, 10.0),
        ],
    ];
    for outline in outlines {
        let positions: Vec<Vec3> = outline.iter().map(|&(x, y)| Vec3::new(x, y, 0.0)).collect();
        let area: f64 = crate::planar::signed_area(outline);
        let tris = crate::planar::triangulate_loops(
            &positions,
            Vec3::new(0.0, 0.0, 1.0),
            vec![(0..outline.len() as u32).collect()],
        )
        .expect("the outline was not triangulated");
        let mut covered = 0.0;
        for t in tris {
            let [a, b, c] = t.map(|i| positions[i as usize]);
            let twice = (b - a).cross(c - a).z;
            assert!(twice > 0.0, "a triangle {a:?} {b:?} {c:?} is folded over");
            covered += twice / 2.0;
        }
        assert!((covered - area).abs() < 1e-9, "{covered} covered of an outline of {area}");
    }
}

#[test]
pub(crate) fn a_plate_drilled_in_a_grid_has_its_faces_rebuilt() {
    // Regression: a 200 mm plate with a 10x10 grid of holes was not rebuilt (86,584 triangles versus
    // 13,212). The first bridge crossed unbridged holes, and bridges from each hole's rightmost point
    // walled the ear clipper into corridors.
    let mut operands = vec![primitives::box_mesh(200.0, 200.0, 5.0)];
    for i in 0..10 {
        for j in 0..10 {
            let at = Vec3::new(-67.5 + 15.0 * i as f64, -67.5 + 15.0 * j as f64, 0.0);
            operands.push(primitives::cylinder_mesh(5.0, 5.0, 20.0, 32).translated(at));
        }
    }
    let result = evaluate_boolean(BooleanOp::Difference, &operands);
    assert_manifold("drilled grid", &result);
    assert_bounds("drilled grid", &result, Vec3::new(200.0, 200.0, 5.0), 1e-9);
    let facing = |up: bool| {
        result
            .indices
            .iter()
            .filter(|t| {
                let [a, b, c] = t.map(|i| result.positions[i as usize]);
                let z = (b - a).cross(c - a).normalized().z;
                if up {
                    z > 0.99
                } else {
                    z < -0.99
                }
            })
            .count()
    };
    // 100 holes of 32 sides and 4 corners is 3402 triangles, plus a few from T-junctions.
    assert!(facing(true) < 4000, "the top face kept {} triangles", facing(true));
    assert!(facing(false) < 4000, "the bottom face kept {} triangles", facing(false));
    // The holes are all still there: the plate less a hundred 32-gons.
    let hole = 100.0 * 0.5 * 32.0 * 2.5f64.powi(2) * (2.0 * std::f64::consts::PI / 32.0).sin();
    let expected = (200.0 * 200.0 - hole) * 5.0;
    assert!((volume(&result) - expected).abs() < 1e-3, "volume {} against {expected}", volume(&result));
}

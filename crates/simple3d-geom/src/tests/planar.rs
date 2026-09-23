//! Rebuilding a flat region from its boundary.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn rebuilding_a_flat_region_keeps_its_boundary() {
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
pub(crate) fn a_region_that_cannot_be_rebuilt_keeps_its_original_triangles() {
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

#[test]
pub(crate) fn copies_meeting_off_centre_leave_no_face_folded_over_the_notch() {
    // Three boxes in a row with the middle one moved back half its depth -- a
    // pattern shifting every other copy. The outline of the bottom face then has
    // an inner corner exactly in line with two outer ones, and the ear clipper
    // took an ear whose new side ran straight through that corner: the face
    // came back closed and of the right volume, but with triangles laid across
    // the notch in front of the middle box, facing up into it, where they drew
    // as a grey wedge of floor that is not there.
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
    // The same outline on its own: two notches' worth of corners in line. Both
    // the fold and, on the wider outline, giving up altogether came from ears
    // whose third side passed through a corner of the loop.
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
    // A 200 mm plate with a ten by ten grid of 5 mm holes. With one to four
    // holes the top and bottom faces were rebuilt to about 36 triangles a hole;
    // with a hundred neither was, and the plate came out at 86 584 triangles
    // where 13 212 describe it -- every one of them carried into the next
    // boolean, the viewport and the export.
    //
    // Two things stood in the way. The first hole's bridge ran from the right
    // column to a far corner of the plate, straight through holes that had not
    // been bridged yet, so the loop crossed itself. And with that fixed, every
    // bridge left from a hole's rightmost point, lined up with a whole row of
    // tangent points, and the ear clipper walled itself into the corridors
    // between the rows. Either one alone sent the face back as it came.
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
    // A polygon with 100 holes of 32 sides and 4 outer corners is 3402
    // triangles, and the T-junction pass may add a few.
    assert!(facing(true) < 4000, "the top face kept {} triangles", facing(true));
    assert!(facing(false) < 4000, "the bottom face kept {} triangles", facing(false));
    // And the holes are all still there: the plate less a hundred 32-gons.
    let hole = 100.0 * 0.5 * 32.0 * 2.5f64.powi(2) * (2.0 * std::f64::consts::PI / 32.0).sin();
    let expected = (200.0 * 200.0 - hole) * 5.0;
    assert!((volume(&result) - expected).abs() < 1e-3, "volume {} against {expected}", volume(&result));
}

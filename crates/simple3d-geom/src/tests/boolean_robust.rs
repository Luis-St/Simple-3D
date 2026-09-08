//! The cases that have broken the kernel before: dense meshes, deep
//! chains, and stack depth.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn a_boolean_result_is_no_denser_than_the_solid_it_describes() {
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

/// Evaluation is deterministic (spec section 5.2), and that has to hold across
/// *processes*, not just within one: the subtree cache key is a content hash,
/// two runs are meant to be comparable, and an exported file is meant to be the
/// same file twice. The hull read its horizon edges back out of a `HashMap`,
/// whose iteration order is seeded randomly per process, so the same two
/// spheres hulled to the same solid with its triangles in a different order
/// every time the application was started.
#[test]
pub(crate) fn a_hull_is_the_same_mesh_every_time_it_is_built() {
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
pub(crate) fn a_round_primitive_unions_without_running_out_of_stack() {
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

#[test]
pub(crate) fn a_finely_tessellated_union_is_manifold_and_no_bigger_than_the_solid() {
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

/// The nine-holed plate again, forty times over, with every input coordinate
/// nudged by a few units in the last place.
///
/// It exists because a boolean's answer is not a continuous function of its
/// input. `sin` and `cos` differ by an ULP between one platform's libm and
/// another's, the BSP amplifies that by nine orders of magnitude where two
/// surfaces meet at a grazing angle, and a repair sized for the ~1e-12mm noise
/// of a *single* boolean then leaves a seam a few microns wide. That is exactly
/// how v0.0.8 built clean on Linux and failed this crate's own manifold check
/// on Windows: nothing was wrong with the code that ran here, and nothing here
/// could see it. Jitter can.
///
/// Seven of these forty were broken when it was written, and six of forty on
/// the kernel before that one -- the fragility is older than either. Collapsing
/// the short edges the weld leaves behind takes it to none.
///
/// Ignored only because it takes half a minute:
///
/// ```text
/// cargo test -p simple3d-geom -- --ignored --nocapture jitter
/// ```
#[test]
#[ignore = "half a minute of arithmetic; run it after touching the kernel"]
pub(crate) fn a_chain_of_booleans_survives_the_last_bits_of_its_input() {
    let mut seed = 0x5eed_u64;
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64) - 1.0
    };
    let jitter = |mesh: &Mesh, next: &mut dyn FnMut() -> f64| {
        let nudge = |v: f64, r: f64| v + v.abs().max(1.0) * r * 4.0 * f64::EPSILON;
        let positions = mesh
            .positions
            .iter()
            .map(|p| Vec3::new(nudge(p.x, next()), nudge(p.y, next()), nudge(p.z, next())))
            .collect();
        Mesh { positions, indices: mesh.indices.clone(), tags: mesh.tags.clone() }
    };

    let mut broken = Vec::new();
    for run in 0..40 {
        let mut operands = vec![primitives::box_mesh(100.0, 20.0, 4.0)];
        for i in 0..9 {
            operands.push(primitives::cylinder_mesh(6.0, 6.0, 20.0, 12).translated(Vec3::new(
                -40.0 + i as f64 * 10.0,
                0.0,
                0.0,
            )));
        }
        let operands: Vec<Mesh> = operands.iter().map(|m| jitter(m, &mut next)).collect();
        let result = evaluate_boolean(BooleanOp::Difference, &operands);
        if let Some(issue) = result.manifold_issue() {
            println!("run {run}: {issue}");
            broken.push(run);
        }
    }
    assert!(broken.is_empty(), "{} of 40 jittered runs came back non-manifold: {broken:?}", broken.len());
}

#[test]
pub(crate) fn a_convex_body_is_recognised_as_splitting_nothing() {
    // What the one-pass build rests on: no face of a convex solid divides any
    // other, and a solid with a dent in it has faces that do.
    assert!(crate::csg_bsp::debug_splits_nothing(&primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 64)));
    assert!(crate::csg_bsp::debug_splits_nothing(&primitives::spherical_cap_mesh(20.0, 10.0, 64)));
    assert!(crate::csg_bsp::debug_splits_nothing(&primitives::cylinder_mesh(20.0, 20.0, 20.0, 64.0 as u32)));
    assert!(!crate::csg_bsp::debug_splits_nothing(&primitives::torus_mesh(30.0, 8.0, 360.0, 64)));
}

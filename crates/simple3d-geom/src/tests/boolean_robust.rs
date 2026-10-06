//! Cases that broke the kernel before: dense meshes, deep chains, and stack depth.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn a_boolean_result_is_no_denser_than_the_solid_it_describes() {
    // BSP clipping by infinite planes slices whole faces; a plate with a hole, slot and boss once
    // came out at ~1500 triangles for a ~230-triangle solid. `repair::heal` rebuilds flat regions,
    // and this pins the budget so chained booleans cannot compound again.
    let plate = primitives::box_mesh(40.0, 20.0, 4.0);
    let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 16).translated(Vec3::new(-12.0, 0.0, 0.0));
    let slot = primitives::box_mesh(8.0, 5.0, 20.0).translated(Vec3::new(12.0, 0.0, 0.0));
    let boss = primitives::cylinder_mesh(9.0, 9.0, 5.0, 16);

    let drilled = evaluate_boolean(BooleanOp::Difference, &[plate, hole, slot]);
    assert_manifold("drilled plate", &drilled);
    let assembly = evaluate_boolean(BooleanOp::Union, &[drilled, boss]);
    assert_manifold("assembly", &assembly);

    // The promised dimensions survive: a 4 mm plate with a 5 mm boss centred on it.
    assert_bounds("assembly", &assembly, Vec3::new(40.0, 20.0, 5.0), 1e-9);
    assert!(
        assembly.triangle_count() < 400,
        "a plate with a hole, a slot and a boss came out at {} triangles",
        assembly.triangle_count()
    );
}

/// Evaluation is deterministic across processes (spec section 5.2). Regression: the hull read its
/// horizon edges from a randomly seeded `HashMap`, reordering triangles per run.
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
    // Regression: a finely segmented sphere touching another body overflowed the stack. A convex
    // body's BSP is a chain one node per face, and the walks were recursive. A quarter megabyte of
    // stack is far less than recursing such a chain needs.
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
    // Regression: a 176-segment cap sunk into a plate gave 2.7 million triangles (a cascading
    // T-junction pass) and was non-manifold. It must match the 64-segment union's volume.
    let plate = || primitives::box_mesh(40.0, 40.0, 4.0);
    let coarse = evaluate_boolean(BooleanOp::Union, &[plate(), primitives::spherical_cap_mesh(20.0, 10.0, 64)]);
    let fine = evaluate_boolean(BooleanOp::Union, &[plate(), primitives::spherical_cap_mesh(20.0, 10.0, 176)]);
    assert_manifold("union at 64 segments", &coarse);
    assert_manifold("union at 176 segments", &fine);

    let (v0, v1) = (volume(&coarse), volume(&fine));
    assert!((v1 - v0).abs() / v0 < 0.01, "volume moved from {v0} to {v1} between tessellations");

    // About ten times the 64-segment size at most; the cascade produced eight hundred times.
    assert!(
        fine.triangle_count() < coarse.triangle_count() * 10,
        "{} triangles at 176 segments against {} at 64",
        fine.triangle_count(),
        coarse.triangle_count()
    );
}

/// The nine-holed plate forty times with inputs jittered by a few ULPs. Platform libm differences
/// get amplified at grazing surfaces into micron seams, which is how v0.0.8 passed on Linux and
/// failed on Windows; jitter reproduces that. Collapsing the weld's short edges fixed all of them.
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
        Mesh { positions, indices: mesh.indices.clone(), tags: mesh.tags.clone(), sources: Vec::new() }
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
    // The one-pass build relies on this: no face of a convex solid divides another; a dented one does.
    assert!(crate::csg_bsp::debug_splits_nothing(&primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 64)));
    assert!(crate::csg_bsp::debug_splits_nothing(&primitives::spherical_cap_mesh(20.0, 10.0, 64)));
    assert!(crate::csg_bsp::debug_splits_nothing(&primitives::cylinder_mesh(20.0, 20.0, 20.0, 64.0 as u32)));
    assert!(!crate::csg_bsp::debug_splits_nothing(&primitives::torus_mesh(30.0, 8.0, 360.0, 64)));
}

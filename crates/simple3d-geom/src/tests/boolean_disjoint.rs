//! Operands that never touch, and the kernel they are meant to skip.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn a_landscape_unions_with_a_shape_that_overlaps_it() {
    // Regression: a landscape unioned with a box was non-manifold and slow, because the landscape's own
    // BSP fragmented its faces. `clip_near` and the parity test replaced the tree.
    let ground = landscape(96, 96);
    assert_manifold("landscape", &ground);

    for (name, at) in [("through the surface", 8.0), ("resting on top", 18.0), ("buried", 2.0)] {
        let cube = primitives::box_mesh(20.0, 20.0, 20.0);
        let lifted = Mesh { positions: cube.positions.iter().map(|p| *p + Vec3::new(0.0, 0.0, at)).collect(), ..cube };
        let out = evaluate_boolean(BooleanOp::Union, &[ground.clone(), lifted]);
        assert_manifold(&format!("landscape + box {name}"), &out);
    }
}

#[test]
pub(crate) fn a_union_survives_an_operand_of_several_separate_solids() {
    // Three disjoint boxes (a linear pattern) unioned with a box overlapping one. Regression: the
    // coincident-face rule asked the wrong triangle of a two-triangle wall, keeping both copies.
    let cube = primitives::box_mesh(20.0, 20.0, 20.0);
    let moved = |x: f64| Mesh {
        positions: cube.positions.iter().map(|p| *p + Vec3::new(x, 0.0, 0.0)).collect(),
        ..cube.clone()
    };
    for &(name, overlapping) in &[("the first", 3.0), ("the second", 33.0), ("the third", 63.0)] {
        let mut pattern = Mesh::new();
        for x in [0.0, 30.0, 60.0] {
            pattern.append(&moved(x));
        }
        let out = evaluate_boolean(BooleanOp::Union, &[pattern, moved(overlapping)]);
        assert_manifold(&format!("three copies + a box overlapping {name}"), &out);
        // Three bodies with the overlapping pair fused: 36 triangles, not 48.
        assert_eq!(out.weld().triangle_count(), 36, "{name}");
    }
}

#[test]
pub(crate) fn a_union_of_scattered_solids_never_reaches_the_kernel() {
    // Folding `union` grows one box over everything so far; `union_all` keeps per-island boxes. This
    // pins the cost: disjoint boxes must stay linear.
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

    // Disjoint operands concatenate, adding or removing no triangles.
    assert_eq!(result.triangle_count(), expected_triangles);
    assert_manifold("scattered union", &result);
    assert!(elapsed.as_secs_f64() < 1.0, "a union of 40 disjoint boxes took {elapsed:?}");
}

#[test]
pub(crate) fn merging_two_islands_still_catches_a_third_that_now_touches() {
    // Merging two islands grows the box into a third: three in a row, fed middle-last, must end as one.
    let left = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(-8.0, 0.0, 0.0));
    let right = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(8.0, 0.0, 0.0));
    let middle = primitives::box_mesh(10.0, 10.0, 10.0);

    let result = evaluate_boolean(BooleanOp::Union, &[left, right, middle]);
    assert_manifold("bridged union", &result);
    // One solid 26 mm long, not three overlapping boxes.
    assert_bounds("bridged union", &result, Vec3::new(26.0, 10.0, 10.0), 1e-9);
}

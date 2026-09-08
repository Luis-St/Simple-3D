//! Operands that never touch, and the kernel they are meant to skip.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn a_landscape_unions_with_a_shape_that_overlaps_it() {
    // Two clicks from an empty document -- add a landscape, add a box -- used to
    // report `Union produced non-manifold geometry` and refuse to export, on
    // half the shipped presets, and take two seconds doing it.
    //
    // The cause was that the landscape was put through a BSP tree of its own
    // planes. Its faces came out of that tree as five times as many fragments,
    // and it
    // was the fragments the union emitted: T-junctions and slivers a hundred
    // millimetres from anything the box came near, more than `repair` could
    // always close. `clip_near` and the parity point test replaced the tree, and
    // the faces now come out of a union as the faces that went in.
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
    // One mesh holding three disjoint boxes -- which is what a linear pattern of
    // three evaluates to -- unioned with a fourth box overlapping one of them.
    //
    // The coincident-face rule was asking the wrong triangle. A box's side wall
    // is two triangles in one plane; a piece resting on that wall lies in the
    // plane of both and on only one of them, and the rule took whichever was
    // found first. Given the other one it had no answer, fell through to a side
    // test on a point that is *on* the surface, and both copies of the shared
    // wall survived.
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
        // Three bodies, and the overlapping pair fused into one: 36 triangles,
        // not 48 with two of them left standing inside each other.
        assert_eq!(out.weld().triangle_count(), 36, "{name}");
    }
}

#[test]
pub(crate) fn a_union_of_scattered_solids_never_reaches_the_kernel() {
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
pub(crate) fn merging_two_islands_still_catches_a_third_that_now_touches() {
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

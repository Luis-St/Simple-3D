//! What the operations evaluate to, and what a failing one reports.

use super::*;
use crate::scene::{GroupOp, Scene};
use simple3d_geom::{Mesh, Vec3};

#[test]
pub(crate) fn a_single_primitive_evaluates_to_its_declared_dimensions() {
    let mut scene = Scene::new();
    let root = scene.root();
    plate(&mut scene, root);
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(out.errors.is_empty());
    let s = size(&out.mesh);
    assert!((s.x - 40.0).abs() < 1e-9 && (s.y - 20.0).abs() < 1e-9 && (s.z - 4.0).abs() < 1e-9, "{s:?}");
}

#[test]
pub(crate) fn a_hole_drilled_through_a_plate_is_watertight_and_in_place() {
    // Spec acceptance criterion 4, through the node tree this time.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    plate(&mut scene, group);
    let hole = cylinder(&mut scene, group, 6.0, 20.0);
    scene.get_mut(hole).unwrap().position = Vec3::new(-8.0, 0.0, 0.0);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(out.mesh.manifold_issue().is_none());
    let s = size(&out.mesh);
    assert!((s.x - 40.0).abs() < 1e-9 && (s.z - 4.0).abs() < 1e-9, "{s:?}");
    for p in &out.mesh.positions {
        let r = ((p.x + 8.0).powi(2) + p.y * p.y).sqrt();
        assert!(r > 3.0 - 1e-6, "a vertex ended up inside the hole");
    }
}

#[test]
pub(crate) fn hiding_a_difference_child_removes_the_cut_and_nothing_else() {
    // Spec acceptance criterion 9.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    plate(&mut scene, group);
    let hole = cylinder(&mut scene, group, 6.0, 20.0);

    let mut evaluator = Evaluator::new();
    let cut = evaluator.evaluate(&scene, &Cancel::new());
    scene.get_mut(hole).unwrap().visible = false;
    let uncut = evaluator.evaluate(&scene, &Cancel::new());

    assert!(cut.mesh.triangle_count() > uncut.mesh.triangle_count());
    let plain = {
        let mut fresh = Scene::new();
        let root = fresh.root();
        plate(&mut fresh, root);
        Evaluator::new().evaluate(&fresh, &Cancel::new())
    };
    assert_eq!(uncut.mesh.triangle_count(), plain.mesh.triangle_count());
}

#[test]
pub(crate) fn evaluation_is_deterministic() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    plate(&mut scene, group);
    cylinder(&mut scene, group, 6.0, 20.0);

    let a = Evaluator::new().evaluate(&scene, &Cancel::new());
    let b = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert_eq!(a.mesh.indices, b.mesh.indices);
    assert_eq!(a.mesh.positions, b.mesh.positions);
}

#[test]
pub(crate) fn a_failing_boolean_names_the_offending_node() {
    // Spec section 5.2: "must fail loudly on that node, naming it in the
    // outliner". A lone triangle is not a solid, so unioning it with itself
    // cannot produce a manifold result -- a deterministic stand-in for the
    // degenerate input a user might build.
    let mut sliver = Mesh::new();
    sliver.push_triangle(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 0.0));
    let mut errors = Vec::new();
    let out = combine(GroupOp::Union, &[sliver.clone(), sliver], 42, "Bad group", &mut errors, &Cancel::new());
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].node, 42);
    assert_eq!(errors[0].name, "Bad group");
    assert!(errors[0].message.contains("Union"), "{}", errors[0].message);
    // The operands still come back so the rest of the scene can preview.
    assert!(out.triangle_count() > 0);
}

#[test]
pub(crate) fn a_failing_boolean_does_not_stop_the_rest_of_the_scene_previewing() {
    // Spec acceptance criterion 15. A needle-thin operand is the kind of
    // input the epsilon-based kernel cannot resolve; whether it fails is up
    // to the kernel, but if it does the healthy plate must still be there
    // and every reported error must name a real node.
    let mut scene = Scene::new();
    let root = scene.root();
    let good = plate(&mut scene, root);
    scene.get_mut(good).unwrap().position = Vec3::new(200.0, 0.0, 0.0);
    let bad = scene.add_group(GroupOp::Hull, root, 1);
    let needle = cylinder(&mut scene, bad, 1e-4, 10.0);
    scene.get_mut(needle).unwrap().segments = Some(8);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    for error in &out.errors {
        assert!(scene.contains(error.node), "error names a node that does not exist");
        assert!(!error.message.is_empty());
        assert_eq!(error.name, scene.node(error.node).name);
    }
    let (_, hi) = out.mesh.bounds().unwrap();
    assert!(hi.x > 180.0, "the healthy plate is missing from the preview");
    let _ = bad;
}

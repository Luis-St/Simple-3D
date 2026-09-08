//! Patterns and splits as the evaluator sees them.

use super::*;
use crate::primitive::ParamValue;
use crate::scene::Scene;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_pattern_repeats_its_child_without_touching_the_original() {
    // Issue 67: a linear pattern of a 20mm box, three copies stepping 50mm
    // along X, spans one box either end of three centres.
    let mut scene = Scene::new();
    let root = scene.root();
    let pat = scene.add_pattern(root, 0);
    let child = scene.add_primitive("box", pat, 0).unwrap();
    let one_box = {
        let mut solo = Scene::new();
        let r = solo.root();
        solo.add_primitive("box", r, 0).unwrap();
        Evaluator::new().evaluate(&solo, &Cancel::new()).mesh.triangle_count()
    };
    {
        let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
        p.insert("kind".into(), ParamValue::Choice(0));
        p.insert("count".into(), ParamValue::Count(3));
        p.insert("step_x".into(), ParamValue::Length(50.0));
    }
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (lo, hi) = out.mesh.bounds().unwrap();
    assert!((lo.x + 10.0).abs() < 1e-6, "the first copy is not the original: {lo:?}");
    assert!((hi.x - 110.0).abs() < 1e-6, "the third copy is not 100mm along: {hi:?}");
    assert_eq!(out.mesh.triangle_count(), one_box * 3, "a pattern of three should be three copies");
    let _ = child;

    // The whole repeated mesh is pickable under the pattern's own id, so a
    // click on any copy reaches the pattern.
    assert!(out.node_meshes.contains_key(&pat), "the pattern has no pickable mesh");
}

/// A split evaluates to its pieces standing side by side, not to a union of
/// them (issue 82).
///
/// The pieces were cut out of one solid by cells that do not overlap, so a
/// union has nothing to resolve -- what it does instead is undo the split:
/// every pair of pieces meets along a whole face, the kernel welds them,
/// and what comes back is the shape they were cut from. A plate cut into
/// 378 squares evaluated to the twelve triangles of the plate, and it took
/// a tenth of a second in release and seconds in a debug build, on every
/// edit of the scene.
#[test]
pub(crate) fn a_split_is_its_pieces_side_by_side_rather_than_a_union_of_them() {
    let mut scene = Scene::new();
    let root = scene.root();
    let shape = scene.add_primitive("box", root, 0).unwrap();
    let original = scene.export_subtree(shape).unwrap();
    scene.remove(shape);
    let split = scene.add_split(original, None, root, 0);
    // Two ten-millimetre cells meeting along a whole face, which is what
    // every pair of pieces of a real split does.
    for (index, x) in [-5.0_f64, 5.0].into_iter().enumerate() {
        let mut mesh = simple3d_geom::primitives::box_mesh(10.0, 10.0, 10.0);
        for p in &mut mesh.positions {
            p.x += x;
        }
        scene.add_mesh(&format!("Piece {index}"), crate::mesh_data::MeshData::new(mesh), split, index);
    }

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(out.errors.is_empty(), "a split reported {:?}", out.errors);
    assert_eq!(out.mesh.triangle_count(), 24, "the pieces were welded back into the shape they came from");
    assert!(out.mesh.manifold_issue().is_none(), "the pieces side by side are not closed");
}

#[test]
pub(crate) fn a_pattern_whose_copies_touch_is_still_one_solid() {
    // Regression, issue 67: the stock step was 20mm and the stock box is
    // 20mm wide, so the very first thing the pattern tool produced was three
    // copies face to face. Concatenated, that welds into an edge shared by
    // four triangles and the enclosing group reported "Union produced
    // non-manifold geometry: edge (13,12) used 2 times, expected 1" -- the
    // whole scene red on the feature's first use. Unioning the copies gives
    // what the manual duplicates a pattern replaces would have given.
    let mut scene = Scene::new();
    let root = scene.root();
    let pat = scene.add_pattern(root, 0);
    scene.add_primitive("box", pat, 0).unwrap();
    {
        let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
        p.insert("kind".into(), ParamValue::Choice(0));
        p.insert("count".into(), ParamValue::Count(3));
        p.insert("step_x".into(), ParamValue::Length(20.0)); // exactly the box's width
    }
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(out.errors.is_empty(), "a touching pattern reported {:?}", out.errors);
    assert!(out.mesh.manifold_issue().is_none(), "the touching copies did not fuse into one solid");
    // Fused end to end, so it spans three boxes and no interior walls are left.
    let (lo, hi) = out.mesh.bounds().unwrap();
    assert!((hi.x - lo.x - 60.0).abs() < 1e-6, "{:?}", hi - lo);
}

#[test]
pub(crate) fn a_pattern_of_copies_that_stand_clear_stays_a_concatenation() {
    // The other half of the bargain: copies with a gap between them must not
    // pay for the boolean kernel, so the result is still exactly the copies.
    let mut scene = Scene::new();
    let root = scene.root();
    let pat = scene.add_pattern(root, 0);
    scene.add_primitive("box", pat, 0).unwrap();
    let one_box = {
        let mut solo = Scene::new();
        let r = solo.root();
        solo.add_primitive("box", r, 0).unwrap();
        Evaluator::new().evaluate(&solo, &Cancel::new()).mesh.triangle_count()
    };
    {
        let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
        p.insert("count".into(), ParamValue::Count(4));
        p.insert("step_x".into(), ParamValue::Length(50.0));
    }
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(out.errors.is_empty());
    assert_eq!(out.mesh.triangle_count(), one_box * 4, "the disjoint copies went through the kernel");
}

#[test]
pub(crate) fn a_grid_pattern_cannot_be_asked_for_more_copies_than_anything_can_draw() {
    // Issue 67: each count clamps to 512 on its own, but a grid multiplies
    // three of them -- 512^3 is 134 million transforms, ~14 GB, reachable by
    // typing three numbers. The cap is what stands between that and an
    // out-of-memory kill.
    let mut scene = Scene::new();
    let root = scene.root();
    let pat = scene.add_pattern(root, 0);
    scene.add_primitive("box", pat, 0).unwrap();
    {
        let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
        p.insert("kind".into(), ParamValue::Choice(1));
        for key in ["grid_x", "grid_y", "grid_z"] {
            p.insert(key.into(), ParamValue::Count(512));
        }
        // Spread out, so the cap is what limits the work and not the kernel.
        for key in ["grid_step_x", "grid_step_y", "grid_step_z"] {
            p.insert(key.into(), ParamValue::Length(40.0));
        }
    }
    let params = scene.node(pat).params().unwrap().clone();
    let (wanted, made) = crate::pattern::instance_count(&params);
    assert_eq!(wanted, 512 * 512 * 512, "the per-axis clamp still multiplies out");
    assert_eq!(made, crate::pattern::MAX_INSTANCES);
    assert_eq!(crate::pattern::instances(&params).len(), crate::pattern::MAX_INSTANCES);
    // And it really evaluates, rather than taking the machine down with it.
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(out.mesh.triangle_count() > 0);
}

#[test]
pub(crate) fn a_mirror_pattern_reflects_its_child_and_stays_watertight() {
    // A box off to +X, mirrored across the X-normal plane: a matching box at
    // -X, and each copy is still a solid despite the reflection's winding.
    let mut scene = Scene::new();
    let root = scene.root();
    let pat = scene.add_pattern(root, 0);
    let child = scene.add_primitive("box", pat, 0).unwrap();
    scene.get_mut(child).unwrap().position = Vec3::new(30.0, 0.0, 0.0);
    {
        let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
        p.insert("kind".into(), ParamValue::Choice(3)); // Mirror
        p.insert("mirror_axis".into(), ParamValue::Choice(0));
    }
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (lo, hi) = out.mesh.bounds().unwrap();
    // Symmetric about the origin: the reflection reached -X.
    assert!((lo.x + hi.x).abs() < 1e-6, "the mirror is not symmetric: {lo:?} {hi:?}");
    // Each of the two copies is a closed solid; a reflection whose winding was
    // not flipped would be inside out.
    assert!(out.mesh.manifold_issue().is_none(), "the reflected copy was left inside out");
}

#[test]
pub(crate) fn hidden_nodes_are_excluded_from_the_result_entirely() {
    let mut scene = Scene::new();
    let root = scene.root();
    let a = plate(&mut scene, root);
    let b = plate(&mut scene, root);
    scene.get_mut(b).unwrap().position = Vec3::new(500.0, 0.0, 0.0);
    scene.get_mut(b).unwrap().visible = false;

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (_, hi) = out.mesh.bounds().unwrap();
    assert!(hi.x < 100.0, "a hidden node contributed geometry");
    // Its own mesh is still available so it can be drawn as a ghost.
    assert!(out.node_meshes.contains_key(&b));
    let _ = a;
}

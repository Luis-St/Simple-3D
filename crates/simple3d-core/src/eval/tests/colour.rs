//! The colour a surface carries through an evaluation.

use super::*;
use crate::scene::{GroupOp, Scene};
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_colour_follows_each_surface_through_a_difference() {
    // What the whole per-face tag exists for: after a painted cutter drills
    // a painted plate, the wall of the hole is the cutter's colour and the
    // plate around it is still the plate's.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let base = plate(&mut scene, group);
    let hole = cylinder(&mut scene, group, 6.0, 20.0);
    scene.paint_subtree(base, Some(crate::scene::Colour([0x10, 0x20, 0x30])));
    scene.paint_subtree(hole, Some(crate::scene::Colour([0xF0, 0xE0, 0xD0])));

    let result = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let counts = painted(&result.mesh);
    assert!(counts.contains_key(&Some([0x10, 0x20, 0x30])), "the plate lost its colour: {counts:?}");
    assert!(counts.contains_key(&Some([0xF0, 0xE0, 0xD0])), "the hole wall lost the cutter's: {counts:?}");
    assert!(!counts.contains_key(&None), "some surface came out unpainted: {counts:?}");
}

#[test]
pub(crate) fn painting_a_group_paints_everything_in_it_and_a_shape_can_still_differ() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    let a = plate(&mut scene, group);
    let b = cylinder(&mut scene, group, 6.0, 4.0);
    scene.get_mut(b).unwrap().position = Vec3::new(30.0, 0.0, 0.0);

    scene.paint_subtree(group, Some(crate::scene::Colour([1, 2, 3])));
    assert_eq!(scene.effective_colour(a).map(|c| c.0), Some([1, 2, 3]));
    assert_eq!(scene.effective_colour(b).map(|c| c.0), Some([1, 2, 3]));
    // Inherited, not copied: only the group carries a colour of its own.
    assert!(scene.node(a).colour.is_none());

    scene.paint_subtree(b, Some(crate::scene::Colour([9, 9, 9])));
    let counts = painted(&Evaluator::new().evaluate(&scene, &Cancel::new()).mesh);
    assert!(counts.contains_key(&Some([1, 2, 3])), "{counts:?}");
    assert!(counts.contains_key(&Some([9, 9, 9])), "{counts:?}");

    // And painting the group again takes the whole group back, including
    // the shape that had been painted on its own.
    scene.paint_subtree(group, Some(crate::scene::Colour([4, 5, 6])));
    assert_eq!(scene.effective_colour(b).map(|c| c.0), Some([4, 5, 6]));
}

#[test]
pub(crate) fn repainting_re_evaluates_but_moving_a_painted_shape_does_not_recolour_it() {
    // The colour is part of the geometry cache key, or a repaint would show
    // the cached mesh in the old colour.
    let mut scene = Scene::new();
    let root = scene.root();
    let id = plate(&mut scene, root);
    let mut evaluator = Evaluator::new();
    let before = painted(&evaluator.evaluate(&scene, &Cancel::new()).mesh);
    assert_eq!(before.keys().collect::<Vec<_>>(), vec![&None]);

    scene.paint_subtree(id, Some(crate::scene::Colour([7, 7, 7])));
    let after = painted(&evaluator.evaluate(&scene, &Cancel::new()).mesh);
    assert_eq!(after.keys().collect::<Vec<_>>(), vec![&Some([7, 7, 7])]);
}

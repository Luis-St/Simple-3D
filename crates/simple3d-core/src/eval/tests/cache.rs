//! What invalidates a cached subtree and what leaves it alone.

use super::*;
use crate::primitive::ParamValue;
use crate::scene::{GroupOp, Scene};
use simple3d_geom::Vec3;

#[test]
pub(crate) fn editing_one_dimension_reuses_the_rest_of_the_cache() {
    // Spec section 5.2: "invalidated only where the tree actually changed".
    let mut scene = Scene::new();
    let root = scene.root();
    let untouched = scene.add_group(GroupOp::Difference, root, 0);
    plate(&mut scene, untouched);
    cylinder(&mut scene, untouched, 6.0, 20.0);
    let edited = scene.add_group(GroupOp::Union, root, 1);
    let target = cylinder(&mut scene, edited, 10.0, 10.0);
    scene.get_mut(edited).unwrap().position = Vec3::new(80.0, 0.0, 0.0);

    let mut evaluator = Evaluator::new();
    evaluator.evaluate(&scene, &Cancel::new());
    let key_untouched = evaluator.subtree_key(&scene, untouched);
    let key_edited = evaluator.subtree_key(&scene, edited);

    scene.get_mut(target).unwrap().params_mut().unwrap().insert("height".into(), ParamValue::Length(11.0));
    assert_eq!(evaluator.subtree_key(&scene, untouched), key_untouched, "untouched subtree was invalidated");
    assert_ne!(evaluator.subtree_key(&scene, edited), key_edited, "edited subtree was not invalidated");
    assert!(evaluator.subtrees.contains_key(&key_untouched), "untouched subtree fell out of the cache");
}

#[test]
pub(crate) fn renaming_does_not_invalidate_anything() {
    let mut scene = Scene::new();
    let root = scene.root();
    let id = plate(&mut scene, root);
    let evaluator = Evaluator::new();
    let before = evaluator.subtree_key(&scene, root);
    scene.get_mut(id).unwrap().name = "Something else".into();
    assert_eq!(evaluator.subtree_key(&scene, root), before);
}

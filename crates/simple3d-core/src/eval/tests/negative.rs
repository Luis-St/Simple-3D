//! Negative parts (issue 119): a shown child cut from its siblings instead of added.

use super::*;
use crate::scene::{GroupOp, Scene};

fn volume(scene: &Scene) -> f64 {
    let out = Evaluator::new().evaluate(scene, &Cancel::new());
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    out.mesh.signed_volume()
}

/// A plate with a 6 mm hole drilled through it by a cylinder marked negative, in `parent`.
fn drilled(scene: &mut Scene, parent: NodeId) -> (NodeId, NodeId) {
    let base = plate(scene, parent);
    let hole = cylinder(scene, parent, 6.0, 20.0);
    scene.get_mut(hole).unwrap().negative = true;
    (base, hole)
}

#[test]
pub(crate) fn a_negative_part_cuts_its_siblings_like_a_difference_group() {
    let mut marked = Scene::new();
    let root = marked.root();
    drilled(&mut marked, root);

    let mut grouped = Scene::new();
    let root = grouped.root();
    let group = grouped.add_group(GroupOp::Difference, root, 0);
    plate(&mut grouped, group);
    cylinder(&mut grouped, group, 6.0, 20.0);

    let mut plain = Scene::new();
    let root = plain.root();
    plate(&mut plain, root);

    let (cut, expected, whole) = (volume(&marked), volume(&grouped), volume(&plain));
    assert!(cut < whole - 1.0, "the hole removed nothing: {cut} of {whole}");
    assert!((cut - expected).abs() < 1e-6, "the cut differs from the difference group's: {cut} vs {expected}");
}

#[test]
pub(crate) fn a_hidden_negative_part_cuts_nothing_and_toggling_it_misses_the_cache() {
    let mut scene = Scene::new();
    let root = scene.root();
    let (_, hole) = drilled(&mut scene, root);
    let mut evaluator = Evaluator::new();
    let cut = evaluator.evaluate(&scene, &Cancel::new()).mesh.signed_volume();

    scene.get_mut(hole).unwrap().negative = false;
    let added = evaluator.evaluate(&scene, &Cancel::new()).mesh.signed_volume();
    assert!(added > cut + 1.0, "unmarking the cylinder kept the cached cut: {added} vs {cut}");

    scene.get_mut(hole).unwrap().negative = true;
    scene.get_mut(hole).unwrap().visible = false;
    let hidden = evaluator.evaluate(&scene, &Cancel::new()).mesh.signed_volume();
    assert!(hidden > cut + 1.0, "a hidden negative part still cut: {hidden} vs {cut}");
}

#[test]
pub(crate) fn a_negative_part_is_never_a_difference_groups_base() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let hole = cylinder(&mut scene, group, 6.0, 20.0);
    scene.get_mut(hole).unwrap().negative = true;
    let base = plate(&mut scene, group);
    assert_eq!(scene.difference_base(group), Some(base));
}

#[test]
pub(crate) fn a_negative_part_in_a_pattern_cuts_every_copy() {
    let mut scene = Scene::new();
    let root = scene.root();
    let pattern = scene.add_pattern(root, 0);
    drilled(&mut scene, pattern);
    {
        let params = scene.get_mut(pattern).unwrap().params_mut().unwrap();
        params.insert("kind".into(), ParamValue::Choice(0));
        params.insert("count".into(), ParamValue::Count(2));
        params.insert("step_x".into(), ParamValue::Length(500.0));
    }
    let mut single = Scene::new();
    let root = single.root();
    drilled(&mut single, root);
    let (two, one) = (volume(&scene), volume(&single));
    assert!((two - 2.0 * one).abs() < 1e-6, "a copy came out without its hole: {two} vs 2 x {one}");
}

#[test]
pub(crate) fn an_exported_part_is_cut_by_a_negative_sibling_and_is_no_body_itself() {
    let mut scene = Scene::new();
    let root = scene.root();
    let (base, hole) = drilled(&mut scene, root);
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let parts = part_meshes(&scene, &[base, hole], &out.node_frames);
    assert_eq!(parts.len(), 1, "the negative part was written as a body");
    assert!((parts[0].mesh.signed_volume() - out.mesh.signed_volume()).abs() < 1e-6);
    let bodies = body_meshes(&scene, &[base, hole], &out.node_frames);
    assert_eq!(bodies.len(), 1);
    assert!((bodies[0].mesh.signed_volume() - out.mesh.signed_volume()).abs() < 1e-6);
}

#[test]
pub(crate) fn the_mark_is_saved_only_when_set() {
    let mut scene = Scene::new();
    let root = scene.root();
    let (base, hole) = drilled(&mut scene, root);
    let json = serde_json::to_string(&scene.export_subtree(base).unwrap()).unwrap();
    assert!(!json.contains("negative"), "{json}");
    let data = scene.export_subtree(hole).unwrap();
    assert!(data.negative);
    let copy = scene.import_subtree(&data, root, 2).unwrap();
    assert!(scene.node(copy).negative);
}

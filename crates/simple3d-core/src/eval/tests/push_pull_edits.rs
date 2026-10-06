//! Push/pull's edits kept on their object (issue 73): taken out, dropped, saved, and carried along.

use super::push_pull::{evaluate, face_at, volume};
use super::*;
use crate::scene::{Body, GroupOp, ObjectEdit, Placing, Scene};
use simple3d_geom::Vec3;

/// A box pushed up by 5 on top and pulled in by 2 underneath, with its volume before either.
fn pushed_box(scene: &mut Scene) -> (NodeId, f64) {
    let root = scene.root();
    let block = scene.add_primitive("box", root, 0).unwrap();
    let out = evaluate(scene);
    let before = volume(&out);
    let (lo, hi) = out.bounds.unwrap();
    let (_, top) = face_at(&out, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, hi.z));
    scene.push_pull(block, &top, 5.0, &out.node_frames).unwrap();
    let out = evaluate(scene);
    let (_, bottom) = face_at(&out, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, lo.z));
    scene.push_pull(block, &bottom, -2.0, &out.node_frames).unwrap();
    (block, before)
}

#[test]
pub(crate) fn an_extracted_edit_becomes_a_node_and_the_model_stays_as_it_was() {
    let mut scene = Scene::new();
    let (block, _) = pushed_box(&mut scene);
    let out = evaluate(&scene);
    let edited = volume(&out);

    let cut = scene.extract_face_edit(block, 1, &out.node_frames).unwrap();
    assert_eq!(scene.node(block).edits.len(), 1);
    assert!(matches!(scene.node(cut).body, Body::Extrusion { .. }));
    assert_eq!(scene.node(cut).name, "Box reduction");
    let group = scene.node(cut).parent.unwrap();
    assert_eq!(scene.node(group).group_op(), Some(GroupOp::Difference));
    assert_eq!(scene.node(group).children, vec![block, cut]);
    let out = evaluate(&scene);
    assert!((volume(&out) - edited).abs() < 1e-6, "{} vs {edited}", volume(&out));

    // Inside the difference, an addition is joined to the box, not added beside the cut.
    let added = scene.extract_face_edit(block, 0, &out.node_frames).unwrap();
    assert!(scene.node(block).edits.is_empty());
    let union = scene.node(added).parent.unwrap();
    assert_eq!(scene.node(union).group_op(), Some(GroupOp::Union));
    assert_eq!(scene.node(union).children, vec![block, added]);
    assert!((volume(&evaluate(&scene)) - edited).abs() < 1e-6);
}

#[test]
pub(crate) fn a_dropped_edit_reverts_its_object() {
    let mut scene = Scene::new();
    let (block, before) = pushed_box(&mut scene);
    let placing = |edit: Option<ObjectEdit>| edit.and_then(|edit| edit.as_push().map(|edit| edit.placing));
    assert_eq!(placing(scene.remove_edit(block, 0)), Some(Placing::Add));
    assert_eq!(placing(scene.remove_edit(block, 0)), Some(Placing::Cut));
    assert!(scene.remove_edit(block, 0).is_none());
    assert!((volume(&evaluate(&scene)) - before).abs() < 1e-6);
}

#[test]
pub(crate) fn the_floor_of_a_pocket_pushes_up_and_pulls_down() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let base = scene.add_primitive("box", group, 0).unwrap();
    let cutter = scene.add_primitive("box", group, 1).unwrap();
    scene.get_mut(cutter).unwrap().scale = Vec3::new(0.5, 0.5, 1.0);
    scene.get_mut(cutter).unwrap().position = Vec3::new(0.0, 0.0, 5.0);
    let out = evaluate(&scene);
    let before = volume(&out);
    let floor = evaluate(&scene).node_meshes[&cutter].bounds().unwrap().0.z;
    let (source, face) = face_at(&out, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, floor));
    assert_eq!(source, source_of(base), "a cut's walls are its base's");
    assert_eq!(scene.push_pull(base, &face, 1.0, &out.node_frames), Some((group, 0)));
    let out = evaluate(&scene);
    let grown = before + face.outline.area();
    assert!((volume(&out) - grown).abs() < 1e-6, "{} vs {grown}", volume(&out));
    // The raised floor is still the base's and one face; pulling it back down cuts after the push.
    let (source, raised) = face_at(&out, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, floor + 1.0));
    assert_eq!(source, source_of(base));
    assert!((raised.outline.area() - face.outline.area()).abs() < 1e-6);
    assert_eq!(scene.push_pull(base, &raised, -3.0, &out.node_frames), Some((group, 1)));
    let deeper = grown - 3.0 * face.outline.area();
    assert!((volume(&evaluate(&scene)) - deeper).abs() < 1e-6, "{} vs {deeper}", volume(&evaluate(&scene)));
}

#[test]
pub(crate) fn an_edit_moves_turns_and_scales_with_its_object() {
    let mut scene = Scene::new();
    let root = scene.root();
    let block = scene.add_primitive("box", root, 0).unwrap();
    scene.get_mut(block).unwrap().rotation = Vec3::new(0.0, 0.0, 30.0);
    scene.get_mut(block).unwrap().scale = Vec3::new(2.0, 1.0, 1.0);
    let out = evaluate(&scene);
    let before = volume(&out);
    let hi = out.bounds.unwrap().1;
    let (_, face) = face_at(&out, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, hi.z));
    scene.push_pull(block, &face, 4.0, &out.node_frames).unwrap();
    let out = evaluate(&scene);
    assert!((volume(&out) - (before + face.outline.area() * 4.0)).abs() < 1e-6, "{}", volume(&out));
    let (lo, hi) = out.bounds.unwrap();

    scene.get_mut(block).unwrap().position = Vec3::new(7.0, 0.0, 0.0);
    scene.get_mut(block).unwrap().scale = Vec3::new(2.0, 1.0, 2.0);
    let moved = evaluate(&scene);
    let (lo2, hi2) = moved.bounds.unwrap();
    assert!((lo2.x - (lo.x + 7.0)).abs() < 1e-6 && (hi2.x - (hi.x + 7.0)).abs() < 1e-6);
    assert!((hi2.z - lo2.z - 2.0 * (hi.z - lo.z)).abs() < 1e-6, "the edit did not scale with the box");
}

#[test]
pub(crate) fn edits_round_trip_through_a_project_file_as_format_five() {
    let mut scene = Scene::new();
    pushed_box(&mut scene);
    let text = crate::project::to_string(&scene);
    assert!(text.contains("\"format\": 5") && text.contains("\"edits\""), "{text}");
    let back = crate::project::from_str(&text).unwrap();
    assert!((volume(&evaluate(&back)) - volume(&evaluate(&scene))).abs() < 1e-9);
}

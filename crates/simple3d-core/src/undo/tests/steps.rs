//! Stepping back and forward over a run of edits.

use super::*;
use crate::primitive::ParamValue;
use crate::scene::Scene;
use crate::scene::{Anchor, GroupOp};
use simple3d_geom::Vec3;

#[test]
pub(crate) fn undo_and_redo_leave_the_camera_exactly_where_it_is() {
    // The camera is part of the saved project and therefore part of every
    // snapshot, but it is not part of what an edit did. Undoing a move must
    // not also throw the view back to where it was looking from.
    let mut scene = Scene::new();
    let mut history = History::new();
    let id = shape(&mut scene);
    history.record(&scene, "Move", None);
    scene.get_mut(id).unwrap().position = Vec3::new(10.0, 0.0, 0.0);

    // The user then orbits and zooms.
    let looking = crate::scene::Camera { yaw: 12.5, pitch: -40.0, distance: 999.0, ..Default::default() };
    scene.camera = looking;

    assert_eq!(history.undo(&mut scene).as_deref(), Some("Move"));
    assert_eq!(scene.camera, looking, "undo moved the camera");
    assert_eq!(scene.node(id).position, Vec3::ZERO, "undo did not restore the model");

    assert_eq!(history.redo(&mut scene).as_deref(), Some("Move"));
    assert_eq!(scene.camera, looking, "redo moved the camera");
    assert_eq!(scene.node(id).position, Vec3::new(10.0, 0.0, 0.0));
}

#[test]
pub(crate) fn twenty_mixed_edits_undo_and_redo_to_the_same_tree() {
    // Spec acceptance criterion 11.
    let mut scene = Scene::new();
    let mut history = History::new();
    let root = scene.root();

    let mut ids = Vec::new();
    for i in 0..6 {
        history.record(&scene, "Add box", None);
        let id = shape(&mut scene);
        scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 10.0, 0.0, 0.0);
        ids.push(id);
    }
    history.record(&scene, "Group", None);
    let group = scene.group_selection(&ids[..3]).unwrap();
    history.record(&scene, "Set operation", None);
    scene.get_mut(group).unwrap().body = crate::scene::Body::Group { op: GroupOp::Difference };
    history.record(&scene, "Reparent", None);
    scene.reparent(ids[3], group, 0).unwrap();
    history.record(&scene, "Rename", None);
    scene.get_mut(ids[4]).unwrap().name = "Renamed".into();
    history.record(&scene, "Hide", None);
    scene.get_mut(ids[5]).unwrap().visible = false;
    history.record(&scene, "Anchor", None);
    scene.get_mut(ids[0]).unwrap().anchor = Anchor::Base;
    history.record(&scene, "Rotate", None);
    scene.get_mut(ids[1]).unwrap().rotation = Vec3::new(0.0, 45.0, 0.0);
    history.record(&scene, "Segments", None);
    scene.get_mut(ids[2]).unwrap().segments = Some(64);
    for i in 0..5 {
        history.record(&scene, "Set width", None);
        scene
            .get_mut(ids[i])
            .unwrap()
            .params_mut()
            .unwrap()
            .insert("width".into(), ParamValue::Length(30.0 + i as f64));
    }
    history.record(&scene, "Duplicate", None);
    scene.duplicate(ids[5]).unwrap();
    history.record(&scene, "Reorder", None);
    scene.reorder(ids[4], -1);
    history.record(&scene, "Delete", None);
    scene.remove(ids[5]);

    let after_all = fingerprint(&scene);
    // Twenty-two edits, comfortably past the criterion's twenty.
    let steps = history.past.len();
    assert!(steps >= 20, "only {steps} snapshots");
    for _ in 0..steps {
        history.undo(&mut scene);
    }
    assert!(!history.can_undo(), "more snapshots than edits");
    assert_eq!(fingerprint(&scene), fingerprint(&Scene::new()), "undoing everything did not empty the scene");
    for _ in 0..steps {
        history.redo(&mut scene);
    }
    assert_eq!(fingerprint(&scene), after_all);
    let _ = root;
}

#[test]
pub(crate) fn cutting_and_pasting_undoes_in_two_steps() {
    // Spec acceptance criterion 22.
    let mut scene = Scene::new();
    let mut history = History::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    let child = shape(&mut scene);
    scene.reparent(child, group, 0).unwrap();
    let other = scene.add_group(GroupOp::Union, root, 1);
    let original = fingerprint(&scene);

    history.record(&scene, "Cut", None);
    let data = scene.export_subtree(group).unwrap();
    scene.remove(group);
    history.record(&scene, "Paste", None);
    scene.import_subtree(&data, other, 0).unwrap();

    history.undo(&mut scene);
    history.undo(&mut scene);
    assert_eq!(fingerprint(&scene), original);
}

#[test]
pub(crate) fn a_new_edit_discards_the_redo_stack() {
    let mut scene = Scene::new();
    let mut history = History::new();
    history.record(&scene, "Add", None);
    shape(&mut scene);
    history.undo(&mut scene);
    assert!(history.can_redo());
    history.record(&scene, "Add", None);
    shape(&mut scene);
    assert!(!history.can_redo());
}

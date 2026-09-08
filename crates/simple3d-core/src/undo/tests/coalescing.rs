//! Which edits join into one step and which do not.

use super::*;
use crate::primitive::ParamValue;
use crate::scene::Scene;

#[test]
pub(crate) fn rapid_edits_to_one_field_are_one_step() {
    // Spec section 7.4: "Rapid edits to one field coalesce".
    let mut scene = Scene::new();
    let mut history = History::new();
    let id = shape(&mut scene);
    let before = fingerprint(&scene);

    for width in [21.0, 22.0, 23.0, 24.0] {
        history.record(&scene, "Set width", Some("param:1:width"));
        scene.get_mut(id).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(width));
    }
    history.undo(&mut scene);
    assert_eq!(fingerprint(&scene), before);
    assert!(!history.can_undo());
}

#[test]
pub(crate) fn edits_to_different_fields_do_not_coalesce() {
    let mut scene = Scene::new();
    let mut history = History::new();
    let id = shape(&mut scene);
    history.record(&scene, "Set width", Some("param:1:width"));
    scene.get_mut(id).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(30.0));
    history.record(&scene, "Set depth", Some("param:1:depth"));
    scene.get_mut(id).unwrap().params_mut().unwrap().insert("depth".into(), ParamValue::Length(30.0));

    history.undo(&mut scene);
    assert_eq!(scene.node(id).params().unwrap()["depth"], ParamValue::Length(20.0));
    assert_eq!(scene.node(id).params().unwrap()["width"], ParamValue::Length(30.0));
    assert!(history.can_undo());
}

#[test]
pub(crate) fn closing_the_run_splits_coalescing() {
    let mut scene = Scene::new();
    let mut history = History::new();
    let id = shape(&mut scene);
    history.record(&scene, "Set width", Some("param:1:width"));
    scene.get_mut(id).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(30.0));
    history.close();
    history.record(&scene, "Set width", Some("param:1:width"));
    scene.get_mut(id).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(40.0));
    assert_eq!(history.past.len(), 2);
}

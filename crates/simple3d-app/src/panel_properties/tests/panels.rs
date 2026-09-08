//! Which panels a selection gets, and the locks and scrubs on them.

use super::*;
use simple3d_core::primitive;
use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::scene::GroupOp;
use simple3d_core::scene::Scene;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_whole_scrub_is_one_undo_step() {
    // Forty snapshots for one drag would make undo useless exactly where it
    // is needed most.
    let mut app = headless_app();
    let (a, b) = two_plates(&mut app);
    let unit = app.unit();
    let param = width_spec();
    let steps = app.history.undo_len();

    for frame in 0..40 {
        scrub_param(&mut app, &[a, b], param, param.kind, unit, 0.5, frame == 0);
    }
    assert_eq!(width_of(&app, a), 60.0, "the scrub did not accumulate");
    assert_eq!(width_of(&app, b), 80.0, "each node scrubs from its own value");
    assert_eq!(app.history.undo_len(), steps + 1, "the drag left more than one step to undo");

    app.history.undo(&mut app.scene);
    assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 60.0), "one undo did not put the drag back");
}

#[test]
pub(crate) fn a_position_scrub_moves_every_selected_node_by_the_same_amount() {
    let mut app = headless_app();
    let (a, b) = two_plates(&mut app);
    app.scene.get_mut(b).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
    for frame in 0..10 {
        scrub_transform(&mut app, &[a, b], 0, 1.0, false, frame == 0);
    }
    assert_eq!(app.scene.node(a).position.x, 10.0);
    assert_eq!(app.scene.node(b).position.x, 20.0);
    app.history.undo(&mut app.scene);
    assert_eq!(app.scene.node(a).position.x, 0.0);
    assert_eq!(app.scene.node(b).position.x, 10.0);
}

#[test]
pub(crate) fn a_selection_of_one_kind_of_shape_gets_a_dimensions_panel_and_a_mixed_one_does_not() {
    let mut app = headless_app();
    let (a, b) = two_plates(&mut app);
    assert_eq!(shared_type(&app, &[a, b]).as_deref(), Some("plate"));
    let root = app.scene.root();
    let sphere = app.scene.add_primitive("sphere", root, 2).unwrap();
    assert_eq!(shared_type(&app, &[a, sphere]), None, "a plate and a sphere share no dimension");
    let group = app.scene.add_group(GroupOp::Union, root, 3);
    assert_eq!(shared_type(&app, &[group]), None, "a group has no dimensions of its own");
}

#[test]
pub(crate) fn every_registry_parameter_maps_to_a_field_kind_the_editor_draws() {
    // The editor has a branch per `ParamKind`; a kind it did not handle would
    // silently render nothing, so check every declared parameter falls into
    // one of them.
    for spec in primitive::REGISTRY {
        for param in spec.params {
            let handled = matches!(
                param.kind,
                ParamKind::Length { .. }
                    | ParamKind::Angle { .. }
                    | ParamKind::Count { .. }
                    | ParamKind::Bool
                    | ParamKind::Choice { .. }
            );
            assert!(handled, "{}.{} has an unhandled kind", spec.type_id, param.key);
        }
    }
}

#[test]
pub(crate) fn a_lock_group_reads_as_locked_when_its_members_agree() {
    let mut scene = Scene::new();
    let root = scene.root();
    let id = scene.add_primitive("sphere", root, 0).unwrap();
    let mut app_scene = scene.clone();
    // Defaults are all 20, so the group starts locked.
    assert!(locked_in(&app_scene, id, 1));
    app_scene.get_mut(id).unwrap().params_mut().unwrap().insert("diameter_y".into(), ParamValue::Length(30.0));
    assert!(!locked_in(&app_scene, id, 1));
    let _ = scene;
}

#[test]
pub(crate) fn only_the_relevant_wall_parameter_is_shown() {
    let spec = primitive::lookup("tube").unwrap();
    let mut params = spec.default_params();
    params.insert("wall_mode".into(), ParamValue::Choice(0));
    let visible: Vec<&str> = spec.params.iter().filter(|p| spec.param_visible(p, &params)).map(|p| p.key).collect();
    assert!(visible.contains(&"wall_thickness"));
    assert!(!visible.contains(&"inner_diameter"));

    params.insert("wall_mode".into(), ParamValue::Choice(1));
    let visible: Vec<&str> = spec.params.iter().filter(|p| spec.param_visible(p, &params)).map(|p| p.key).collect();
    assert!(visible.contains(&"inner_diameter"));
    assert!(!visible.contains(&"wall_thickness"));
}

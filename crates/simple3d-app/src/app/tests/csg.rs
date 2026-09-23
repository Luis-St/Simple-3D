//! The boolean a drag of one of its operands is drawn from on the card.

use super::*;
use crate::app::{CSG_DIFFERENCE, CSG_UNION};
use simple3d_core::primitive::ParamValue;
use simple3d_core::scene::{GroupOp, NodeId};
use simple3d_geom::Vec3;

fn cylinder(app: &mut App, parent: NodeId, at: Vec3) -> NodeId {
    let index = app.scene.node(parent).children.len();
    let id = app.scene.add_primitive("cylinder", parent, index).unwrap();
    let params = app.scene.get_mut(id).unwrap().params_mut().unwrap();
    params.insert("diameter_x".into(), ParamValue::Length(4.0));
    params.insert("diameter_y".into(), ParamValue::Length(4.0));
    params.insert("height".into(), ParamValue::Length(20.0));
    app.scene.get_mut(id).unwrap().position = at;
    id
}

fn plate(app: &mut App, parent: NodeId) -> NodeId {
    let index = app.scene.node(parent).children.len();
    app.scene.add_primitive("plate", parent, index).unwrap()
}

fn evaluated(app: &mut App) {
    app.reevaluate_for_test();
    app.scene_renderable = crate::render::Renderable::prepare_scene(&app.evaluated.mesh, &app.evaluated.ranges);
}

#[test]
pub(crate) fn a_cutter_is_drawn_as_the_difference_it_is_in() {
    let mut app = app_in(temp_config_dir("csg-cutter"));
    let root = app.scene.root();
    let drilled = app.scene.add_group(GroupOp::Difference, root, 0);
    let base = plate(&mut app, drilled);
    let first = cylinder(&mut app, drilled, Vec3::new(-5.0, 0.0, 0.0));
    let second = cylinder(&mut app, drilled, Vec3::new(5.0, 0.0, 0.0));
    evaluated(&mut app);

    let (group, leaves, program) = app.csg_plan(first).expect("a cutter's difference can be drawn per pixel");
    assert_eq!(group, drilled);
    assert_eq!(leaves, vec![base, first, second]);
    assert_eq!(program, vec![0, 1, CSG_DIFFERENCE, 2, CSG_DIFFERENCE]);
}

#[test]
pub(crate) fn a_body_inside_a_union_inside_a_difference_opens_both() {
    let mut app = app_in(temp_config_dir("csg-nested"));
    let root = app.scene.root();
    let drilled = app.scene.add_group(GroupOp::Difference, root, 0);
    let body = app.scene.add_group(GroupOp::Union, drilled, 0);
    let base = plate(&mut app, body);
    let boss = cylinder(&mut app, body, Vec3::ZERO);
    let hole = cylinder(&mut app, drilled, Vec3::new(8.0, 0.0, 0.0));
    evaluated(&mut app);

    let (group, leaves, program) = app.csg_plan(boss).expect("the boss's union and difference are both drawable");
    assert_eq!(group, drilled);
    assert_eq!(leaves, vec![base, boss, hole]);
    assert_eq!(program, vec![0, 1, CSG_UNION, 2, CSG_DIFFERENCE]);
}

#[test]
pub(crate) fn a_hull_on_the_way_up_waits_for_the_evaluation() {
    let mut app = app_in(temp_config_dir("csg-hull"));
    let root = app.scene.root();
    let drilled = app.scene.add_group(GroupOp::Difference, root, 0);
    plate(&mut app, drilled);
    let hull = app.scene.add_group(GroupOp::Hull, drilled, 1);
    let inside = cylinder(&mut app, hull, Vec3::ZERO);
    cylinder(&mut app, hull, Vec3::new(3.0, 0.0, 0.0));
    evaluated(&mut app);

    assert!(app.csg_plan(inside).is_none(), "a hull cannot be drawn per pixel");
    // The hull as a whole is a shape of the difference, and moving it is.
    assert!(app.csg_plan(hull).is_some());
}

#[test]
pub(crate) fn a_body_of_its_own_is_left_to_the_plain_live_drag() {
    let mut app = app_in(temp_config_dir("csg-own"));
    let root = app.scene.root();
    let alone = plate(&mut app, root);
    evaluated(&mut app);

    assert!(app.scene_renderable.parts.contains_key(&alone));
    assert!(app.csg_plan(alone).is_none());
}

//! The boolean a drag of one of its operands is drawn from on the card.

use super::*;
use crate::app::{PlannedLeaf, Shape, CSG_DIFFERENCE, CSG_UNION};
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

/// The nodes an expression's shapes are, for a plan with no hull in it.
fn nodes(leaves: &[PlannedLeaf]) -> Vec<NodeId> {
    leaves
        .iter()
        .map(|leaf| match leaf.shape {
            Shape::Node(id) => id,
            Shape::Hull(id) => panic!("{id:?} is a hull"),
        })
        .collect()
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
    assert_eq!(nodes(&leaves), vec![base, first, second]);
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
    assert_eq!(nodes(&leaves), vec![base, boss, hole]);
    assert_eq!(program, vec![0, 1, CSG_UNION, 2, CSG_DIFFERENCE]);
}

#[test]
pub(crate) fn a_hull_on_the_way_up_is_one_shape_made_again_for_the_drag() {
    let mut app = app_in(temp_config_dir("csg-hull"));
    let root = app.scene.root();
    let drilled = app.scene.add_group(GroupOp::Difference, root, 0);
    let base = plate(&mut app, drilled);
    let hull = app.scene.add_group(GroupOp::Hull, drilled, 1);
    let inside = cylinder(&mut app, hull, Vec3::ZERO);
    cylinder(&mut app, hull, Vec3::new(3.0, 0.0, 0.0));
    evaluated(&mut app);

    let (group, leaves, program) = app.csg_plan(inside).expect("a hull over the dragged body is drawn as a shape");
    assert_eq!(group, drilled);
    let shapes: Vec<Shape> = leaves.iter().map(|leaf| leaf.shape).collect();
    assert_eq!(shapes, vec![Shape::Node(base), Shape::Hull(hull)]);
    assert_eq!(program, vec![0, 1, CSG_DIFFERENCE]);
    // The hull as a whole is a shape of the difference, and moving it is.
    assert!(app.csg_plan(hull).is_some());

    // Stretched over where the dragged body has got to.
    let still = app.csg_hull(hull, inside, simple3d_core::xform::Xform::IDENTITY).unwrap();
    let moved = app.csg_hull(hull, inside, simple3d_core::xform::Xform::from_translation(Vec3::new(20.0, 0.0, 0.0)));
    let (_, still_hi) = still.mesh.bounds().unwrap();
    let (_, moved_hi) = moved.unwrap().mesh.bounds().unwrap();
    assert!(
        (moved_hi.x - still_hi.x - 17.0).abs() < 1e-6,
        "the hull did not follow the drag: {still_hi:?} {moved_hi:?}"
    );
}

#[test]
pub(crate) fn a_hull_over_a_difference_waits_for_the_evaluation() {
    let mut app = app_in(temp_config_dir("csg-hull-difference"));
    let root = app.scene.root();
    let hull = app.scene.add_group(GroupOp::Hull, root, 0);
    let drilled = app.scene.add_group(GroupOp::Difference, hull, 0);
    plate(&mut app, drilled);
    let hole = cylinder(&mut app, drilled, Vec3::ZERO);
    cylinder(&mut app, hull, Vec3::new(30.0, 0.0, 0.0));
    evaluated(&mut app);

    assert!(app.csg_plan(hole).is_none(), "a hull is not the hull of its operands' points across a difference");
}

#[test]
pub(crate) fn a_pattern_on_the_way_up_is_drawn_as_its_copies() {
    let mut app = app_in(temp_config_dir("csg-pattern"));
    let root = app.scene.root();
    let drilled = app.scene.add_group(GroupOp::Difference, root, 0);
    let base = plate(&mut app, drilled);
    let pattern = app.scene.add_pattern(drilled, 1);
    let hole = cylinder(&mut app, pattern, Vec3::ZERO);
    evaluated(&mut app);

    let copies = simple3d_core::pattern::instances(&simple3d_core::pattern::default_params()).len();
    assert!(copies > 1, "the default pattern repeats nothing");
    let (group, leaves, program) = app.csg_plan(hole).expect("a pattern's copies are drawn per pixel");
    assert_eq!(group, drilled);
    assert_eq!(leaves.len(), 1 + copies);
    assert_eq!(nodes(&leaves[1..]), vec![hole; copies]);
    assert!(leaves[1..].iter().all(|leaf| leaf.carried && leaf.copy.is_some()));
    assert_eq!(leaves[0], PlannedLeaf { shape: Shape::Node(base), copy: None, carried: false });
    let mut expected = vec![0, 1];
    for index in 2..=copies {
        expected.extend([index as i32, CSG_UNION]);
    }
    expected.push(CSG_DIFFERENCE);
    assert_eq!(program, expected);
    // Each copy where the evaluation put it.
    let unit = app.evaluated.result_mesh(hole).unwrap().bounds().unwrap().0;
    let result = app.evaluated.result_mesh(pattern).unwrap().bounds().unwrap();
    let last = leaves.last().unwrap().copy.unwrap().point(unit);
    assert!(last.x <= result.1.x + 1e-6 && last.x > unit.x, "the last copy is not where the pattern puts it");
}

#[test]
pub(crate) fn a_difference_of_more_than_thirty_one_shapes_is_drawn_per_pixel() {
    let mut app = app_in(temp_config_dir("csg-many"));
    let root = app.scene.root();
    let drilled = app.scene.add_group(GroupOp::Difference, root, 0);
    plate(&mut app, drilled);
    let holes: Vec<NodeId> =
        (0..40).map(|i| cylinder(&mut app, drilled, Vec3::new(i as f64 - 20.0, 0.0, 0.0))).collect();
    evaluated(&mut app);

    let (_, leaves, _) = app.csg_plan(holes[7]).expect("forty cutters fit the masks");
    assert_eq!(leaves.len(), 41);
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

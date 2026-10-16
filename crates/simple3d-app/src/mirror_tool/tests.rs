use super::*;
use crate::app::tests::two_boxes;
use simple3d_core::scene::Node;
use simple3d_core::xform::Xform;

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-7
}

#[test]
fn across_the_x_axis_the_plane_stands_upright_and_flips_y() {
    let tool = MirrorTool::on(vec![1]);
    let (point, normal) = tool.plane().unwrap();
    assert_eq!(point, Vec3::ZERO);
    assert!(close(normal.cross(v(0.0, 1.0, 0.0)), Vec3::ZERO), "{normal:?}");
}

#[test]
fn a_line_along_the_chosen_direction_takes_the_one_most_across_it() {
    let mut tool = MirrorTool::on(vec![1]);
    tool.by = MirrorBy::Line;
    tool.ends = [Some(v(5.0, 0.0, 0.0)), Some(v(5.0, 0.0, 20.0))];
    assert_ne!(tool.holds(), 2, "a vertical line cannot make a plane with Z");
    let (_, normal) = tool.plane().unwrap();
    assert!(normal.z.abs() < 1e-9 && (normal.length() - 1.0).abs() < 1e-9, "{normal:?}");
}

#[test]
fn a_line_without_its_end_says_what_is_missing() {
    let mut tool = MirrorTool::on(vec![1]);
    tool.by = MirrorBy::Line;
    tool.ends = [Some(v(0.0, 0.0, 0.0)), None];
    assert!(tool.plane().unwrap_err().contains("line's end"));
}

#[test]
fn opening_with_nothing_selected_chooses_the_objects_first_from_the_selection() {
    let (mut app, a, b) = two_boxes("mirror-choose", v(40.0, 0.0, 0.0));
    app.clear_selection();
    app.toggle_mirror_tool();
    assert!(app.mirror_tool.as_ref().unwrap().choosing);
    app.select_only(b);
    app.toggle_selected(a);
    app.refresh_mirror_tool();
    let mut targets = app.mirror_tool.as_ref().unwrap().targets.clone();
    targets.sort_unstable();
    assert_eq!(targets, vec![a.min(b), a.max(b)]);
}

#[test]
fn applying_makes_a_mirrored_copy_on_the_far_side_of_a_drawn_line() {
    let (mut app, _, b) = two_boxes("mirror-copy", v(40.0, 15.0, 0.0));
    app.select_only(b);
    app.reevaluate_for_test();
    app.toggle_mirror_tool();
    {
        let tool = app.mirror_tool.as_mut().unwrap();
        assert!(!tool.choosing, "the selection was not taken");
        tool.by = MirrorBy::Line;
        tool.ends = [Some(v(0.0, 0.0, 0.0)), Some(v(100.0, 0.0, 0.0))];
    }
    assert_eq!(templates(&app).len(), 1, "no mirror image is drawn");
    let before = app.scene.depth_first().len();
    app.apply_mirror();
    assert!(app.mirror_tool.is_none(), "the window stayed open after Apply");
    assert_eq!(app.scene.depth_first().len(), before + 1, "no copy was made");
    let copy = app.primary().unwrap();
    assert_ne!(copy, b, "the copy is not what is left selected");
    let node = app.scene.node(copy);
    assert!(close(node.position, v(40.0, -15.0, 0.0)), "{:?}", node.position);
    assert_eq!(Node::sane_scale(node.scale), v(1.0, -1.0, 1.0));
    assert!(close(app.scene.node(b).position, v(40.0, 15.0, 0.0)), "the original moved");
    app.reevaluate_for_test();
    let mesh = app.evaluated.node_meshes.get(&copy).expect("the copy has a body");
    assert!(mesh.signed_volume() > 0.0, "the mirrored box is inside out");
}

#[test]
fn without_keeping_the_originals_the_object_itself_is_mirrored() {
    let (mut app, _, b) = two_boxes("mirror-move", v(40.0, 15.0, 0.0));
    app.select_only(b);
    app.reevaluate_for_test();
    app.toggle_mirror_tool();
    {
        let tool = app.mirror_tool.as_mut().unwrap();
        tool.keep = false;
        tool.axis = 1;
        tool.upright = 2;
    }
    let before = app.scene.depth_first().len();
    app.apply_mirror();
    assert_eq!(app.scene.depth_first().len(), before);
    assert!(close(app.scene.node(b).position, v(-40.0, 15.0, 0.0)), "{:?}", app.scene.node(b).position);
    let mirror = Xform::reflection(Vec3::ZERO, v(1.0, 0.0, 0.0));
    assert!(mirror.mirrors());
}

#[test]
fn the_window_draws_while_choosing_and_with_a_line() {
    let (mut app, _, b) = two_boxes("mirror-draws", v(40.0, 15.0, 0.0));
    app.clear_selection();
    app.toggle_mirror_tool();
    crate::app::tests::draw_one_frame(&mut app);
    app.select_only(b);
    crate::app::tests::draw_one_frame(&mut app);
    {
        let tool = app.mirror_tool.as_mut().unwrap();
        tool.choosing = false;
        tool.by = MirrorBy::Line;
        tool.ends = [Some(v(0.0, 0.0, 0.0)), Some(v(0.0, 0.0, 30.0))];
    }
    crate::app::tests::draw_one_frame(&mut app);
    assert!(app.mirror_tool.is_some(), "drawing closed the tool");
}

#[test]
fn across_an_axis_the_objects_follow_the_selection() {
    let (mut app, a, b) = two_boxes("mirror-follow", v(40.0, 0.0, 0.0));
    app.select_only(a);
    app.toggle_mirror_tool();
    app.select_only(b);
    app.refresh_mirror_tool();
    assert_eq!(app.mirror_tool.as_ref().unwrap().targets, vec![b], "the axis mirror kept the old selection");
    // Across a line the viewport's clicks pick the line, so the objects stay put.
    app.mirror_tool.as_mut().unwrap().by = MirrorBy::Line;
    app.select_only(a);
    app.refresh_mirror_tool();
    assert_eq!(app.mirror_tool.as_ref().unwrap().targets, vec![b], "the line mirror followed the selection");
}

#[test]
fn changing_the_objects_starts_from_the_chosen_ones() {
    let (mut app, _, b) = two_boxes("mirror-change", v(40.0, 0.0, 0.0));
    app.select_only(b);
    app.toggle_mirror_tool();
    app.mirror_tool.as_mut().unwrap().by = MirrorBy::Edge;
    app.clear_selection();
    app.refresh_mirror_tool();
    app.toggle_mirror_choosing();
    app.refresh_mirror_tool();
    let tool = app.mirror_tool.as_ref().unwrap();
    assert!(tool.choosing);
    assert_eq!(tool.targets, vec![b], "pressing Change dropped the objects");
    assert_eq!(app.selection, vec![b], "the chosen objects were not selected to start from");
}

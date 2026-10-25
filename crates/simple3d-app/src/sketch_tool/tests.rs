use super::constrain::Rule;
use super::*;
use crate::app::tests::{draw_one_frame, headless_app, two_boxes};
use crate::app::App;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::ParamValue;

fn click(tool: &mut SketchTool, at: [f64; 2]) {
    let point = tool.sketch.points.iter().find(|p| (p.at[0] - at[0]).hypot(p.at[1] - at[1]) < 1e-9).map(|p| p.id);
    tool.click(Click { at, point });
}

/// An open editor on the ground, with nothing in the scene but the headless plate.
fn sketching() -> App {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    app.place_sketch(Xform::from_translation(Vec3::ZERO), None);
    app
}

/// A 10 x 20 rectangle drawn with the rectangle tool and its region picked, as clicks would.
fn draw_and_pick(app: &mut App) {
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Rectangle);
    click(tool, [100.0, 100.0]);
    click(tool, [110.0, 120.0]);
    let regions = simple3d_sketch::regions::find(&tool.sketch, 64);
    let index = regions.at([105.0, 105.0]).expect("the rectangle encloses a region");
    let pick = regions.pick(index, [105.0, 105.0]);
    tool.change(|sketch| sketch.regions.push(pick));
}

#[test]
fn a_rectangle_drawn_and_picked_becomes_an_extruded_sketch_node_in_one_undo_step() {
    let mut app = sketching();
    draw_and_pick(&mut app);
    let tool = app.sketch_tool.as_ref().unwrap();
    assert_eq!(tool.sketch.curves.len(), 4);
    assert_eq!(tool.sketch.constraints.len(), 4, "a rectangle keeps square by its constraints");
    let steps = app.history.undo_len();
    app.finish_sketch_tool();
    assert!(app.sketch_tool.is_none(), "Finish left the editor open");
    assert_eq!(app.history.undo_len(), steps + 1);
    let id = app.primary().expect("the new sketch is selected");
    assert!(app.scene.node(id).is_sketch());
    app.reevaluate_for_test();
    let bounds = app.evaluated.node_world_bounds.get(&id).copied().expect("the sketch made a solid");
    assert!((bounds.1.x - bounds.0.x - 10.0).abs() < 1e-6 && (bounds.1.z - bounds.0.z - 10.0).abs() < 1e-6);
    app.history.undo(&mut app.scene);
    assert!(!app.scene.contains(id), "undo left the sketch");
}

#[test]
fn nothing_can_be_kept_before_a_plane_and_a_region_are_picked() {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    app.finish_sketch_tool();
    assert!(app.sketch_tool.is_some(), "Finish without a plane closed the editor");
    app.place_sketch(Xform::from_translation(Vec3::ZERO), None);
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Circle);
    click(tool, [0.0, 0.0]);
    click(tool, [5.0, 0.0]);
    app.finish_sketch_tool();
    assert!(app.sketch_tool.is_some(), "Finish without a picked region closed the editor");
}

#[test]
fn undo_and_delete_while_sketching_work_on_the_sketch_not_the_document() {
    let mut app = sketching();
    let steps = app.history.undo_len();
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Line);
    click(tool, [0.0, 0.0]);
    click(tool, [10.0, 0.0]);
    click(tool, [10.0, 10.0]);
    assert_eq!(tool.sketch.curves.len(), 2, "a chain of three clicks is two lines");
    app.run(Command::Undo);
    assert_eq!(app.sketch_tool.as_ref().unwrap().sketch.curves.len(), 1);
    assert_eq!(app.history.undo_len(), steps, "the document's undo was touched");
    let tool = app.sketch_tool.as_mut().unwrap();
    let line = tool.sketch.curves[0].id;
    tool.selection = vec![Pick::Curve(line)];
    app.run(Command::Delete);
    assert!(app.sketch_tool.as_ref().unwrap().sketch.curves.is_empty());
    assert!(app.scene.contains(app.scene.node(app.scene.root()).children[0]), "delete removed the plate");
}

#[test]
fn a_line_chain_closes_back_onto_its_first_point() {
    let mut app = sketching();
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Line);
    for at in [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 0.0]] {
        click(tool, at);
    }
    assert_eq!(tool.sketch.curves.len(), 3);
    assert_eq!(tool.sketch.points.len(), 3, "the closing click made a new point");
    assert!(tool.clicks.is_empty(), "closing the chain did not end it");
    assert_eq!(simple3d_sketch::regions::find(&tool.sketch, 64).list.len(), 1);
}

#[test]
fn a_constraint_button_fits_the_selection_and_a_repeat_is_refused_with_the_other_marked() {
    let mut app = sketching();
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Line);
    click(tool, [0.0, 0.0]);
    click(tool, [10.0, 1.0]);
    tool.end_gesture();
    let line = tool.sketch.curves[0].id;
    assert!(tool.kinds_for(Rule::Horizontal).is_none(), "Horizontal fitted an empty selection");
    tool.selection = vec![Pick::Curve(line)];
    tool.apply(Rule::Horizontal).unwrap();
    let held = tool.sketch.constraints[0].id;
    assert!(tool.apply(Rule::Horizontal).is_err());
    assert_eq!(tool.conflicts, vec![held]);
    tool.apply(Rule::Dimension).unwrap();
    assert!(matches!(tool.selection[..], [Pick::Constraint(_)]), "the new dimension is not selected");
}

#[test]
fn a_cut_with_no_object_under_it_is_a_negative_part_and_revolving_needs_an_axis() {
    let mut app = sketching();
    draw_and_pick(&mut app);
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.placing = Placing::Cut;
    tool.params.insert("feature".into(), ParamValue::Choice(1));
    assert!(super::open::finish_blocked(tool).is_some(), "a revolve without an axis could be kept");
    tool.params.insert("feature".into(), ParamValue::Choice(0));
    app.finish_sketch_tool();
    let id = app.primary().unwrap();
    assert!(app.scene.node(id).negative, "the cut is not a negative part");
}

#[test]
fn a_sketch_on_a_face_cuts_the_object_under_it_and_reopens_on_its_plane() {
    let (mut app, a, _) = two_boxes("sketch-face", Vec3::new(60.0, 0.0, 0.0));
    app.reevaluate_for_test();
    app.selection.clear();
    app.toggle_sketch_tool();
    let (_, hi) = app.evaluated.node_world_bounds[&a];
    let frame =
        super::pointer::plane_frame(Vec3::new(0.0, 0.0, hi.z), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    app.place_sketch(frame, Some(a));
    draw_and_pick(&mut app);
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.placing = Placing::Cut;
    tool.params.insert("distance".into(), ParamValue::Length(2.0));
    tool.params.insert("flip".into(), ParamValue::Bool(true));
    app.finish_sketch_tool();
    let id = app.primary().unwrap();
    let parent = app.scene.node(id).parent.unwrap();
    assert_eq!(app.scene.node(parent).group_op(), Some(simple3d_core::scene::GroupOp::Difference));
    app.reevaluate_for_test();
    app.toggle_sketch_tool();
    let tool = app.sketch_tool.as_ref().expect("the selected sketch did not open");
    assert_eq!(tool.editing, Some(id));
    let origin = tool.frame.unwrap().t;
    assert!((origin.z - hi.z).abs() < 1e-6, "reopened at {origin:?}");
    assert!(app.grid_plane().is_some_and(|grid| (grid.origin.z - hi.z).abs() < 1e-6), "the grid is not the sketch's");
}

#[test]
fn the_window_and_overlays_draw_in_every_state() {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    draw_one_frame(&mut app);
    app.place_sketch(Xform::from_translation(Vec3::ZERO), None);
    draw_and_pick(&mut app);
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Spline);
    click(tool, [0.0, 0.0]);
    click(tool, [5.0, 5.0]);
    tool.selection = vec![Pick::Curve(tool.sketch.curves[0].id)];
    draw_one_frame(&mut app);
    assert!(app.sketch_tool.is_some());
}

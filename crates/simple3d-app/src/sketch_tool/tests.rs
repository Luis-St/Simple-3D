use super::constrain::Rule;
use super::*;
use crate::app::tests::{draw_one_frame, headless_app, two_boxes};
use crate::app::App;
use simple3d_core::keymap::Command;

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

/// A 10 x 20 rectangle drawn with the rectangle tool, as clicks would.
pub(crate) fn draw_rectangle(app: &mut App) {
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Rectangle);
    click(tool, [100.0, 100.0]);
    click(tool, [110.0, 120.0]);
}

#[test]
fn a_rectangle_drawn_becomes_a_flat_sketch_node_in_one_undo_step() {
    let mut app = sketching();
    draw_rectangle(&mut app);
    let tool = app.sketch_tool.as_ref().unwrap();
    assert_eq!(tool.sketch.curves.len(), 4);
    assert_eq!(tool.sketch.constraints.len(), 4, "a rectangle keeps square by its constraints");
    let steps = app.history.undo_len();
    app.finish_sketch_tool();
    assert!(app.sketch_tool.is_none(), "Finish left the editor open");
    assert_eq!(app.history.undo_len(), steps + 1);
    let id = app.primary().expect("the new sketch is selected");
    assert!(app.scene.node(id).is_sketch());
    let before = app.evaluated.mesh.triangle_count();
    app.reevaluate_for_test();
    assert_eq!(app.evaluated.mesh.triangle_count(), before, "the sketch made a solid of its own");
    app.history.undo(&mut app.scene);
    assert!(!app.scene.contains(id), "undo left the sketch");
}

#[test]
fn nothing_can_be_kept_before_a_plane_is_picked_and_something_drawn() {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    app.finish_sketch_tool();
    assert!(app.sketch_tool.is_some(), "Finish without a plane closed the editor");
    app.place_sketch(Xform::from_translation(Vec3::ZERO), None);
    app.finish_sketch_tool();
    assert!(app.sketch_tool.is_some(), "Finish with nothing drawn closed the editor");
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Line);
    click(tool, [0.0, 0.0]);
    click(tool, [5.0, 0.0]);
    app.finish_sketch_tool();
    assert!(app.sketch_tool.is_none(), "an open line could not be kept");
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
fn a_constraint_fits_the_selection_and_a_repeat_is_refused_with_the_other_marked() {
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
}

#[test]
fn a_sketch_on_a_face_reopens_on_its_plane_and_changes_its_solids_when_kept() {
    let (mut app, a, _) = two_boxes("sketch-face", Vec3::new(60.0, 0.0, 0.0));
    app.reevaluate_for_test();
    app.selection.clear();
    app.toggle_sketch_tool();
    let (_, hi) = app.evaluated.node_world_bounds[&a];
    let frame =
        super::plane_pick::plane_frame(Vec3::new(0.0, 0.0, hi.z), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    app.place_sketch(frame, Some(a));
    draw_rectangle(&mut app);
    app.finish_sketch_tool();
    let id = app.primary().unwrap();
    assert_eq!(app.scene.node(id).parent, Some(app.scene.root()), "the sketch is not at the top of the tree");
    app.reevaluate_for_test();
    // A solid made from it, then the sketch opened again and made wider.
    let sketch = (**app.scene.node(id).sketch().unwrap()).clone();
    let regions = simple3d_sketch::regions::find(&sketch, 64);
    let pick = regions.pick(regions.at([105.0, 105.0]).unwrap(), [105.0, 105.0]);
    let feature = simple3d_core::scene::Feature {
        profile: simple3d_core::scene::Profile::Sketch { sketch, regions: vec![pick] },
        axis: None,
    };
    let params = simple3d_core::scene::feature_params(false, &Default::default());
    let frames = app.evaluated.node_frames.clone();
    let solid = app.scene.add_feature("Extrusion", feature, params, &frame, None, &frames).unwrap();
    app.select_only(id);
    app.toggle_sketch_tool();
    let tool = app.sketch_tool.as_mut().expect("the selected sketch did not open");
    assert_eq!(tool.editing, Some(id));
    let origin = tool.frame.unwrap().t;
    assert!((origin.z - hi.z).abs() < 1e-6, "reopened at {origin:?}");
    assert!(app.grid_plane().is_some_and(|grid| (grid.origin.z - hi.z).abs() < 1e-6), "the grid is not the sketch's");
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.change(|sketch| {
        for point in sketch.points.iter_mut().filter(|p| p.at[0] > 105.0) {
            point.at[0] = 130.0;
        }
    });
    let steps = app.history.undo_len();
    app.finish_sketch_tool();
    assert_eq!(app.history.undo_len(), steps + 1, "the change is not one undo step");
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.node_world_bounds[&solid];
    assert!((hi.x - lo.x - 30.0).abs() < 1e-6, "the solid is {} wide, not following its sketch", hi.x - lo.x);
}

#[test]
fn the_window_and_overlays_draw_in_every_state() {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    draw_one_frame(&mut app);
    app.place_sketch(Xform::from_translation(Vec3::ZERO), None);
    draw_rectangle(&mut app);
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Spline);
    click(tool, [0.0, 0.0]);
    click(tool, [5.0, 5.0]);
    tool.selection = vec![Pick::Curve(tool.sketch.curves[0].id)];
    draw_one_frame(&mut app);
    assert!(app.sketch_tool.is_some());
}

#[test]
fn a_click_on_an_orange_origin_plane_puts_the_sketch_on_it_and_faces_it() {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    // Looking in from the positive corner, where none of the squares covers another.
    app.scene.camera.yaw = 45.0;
    app.scene.camera.pitch = 30.0;
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    for (plane, normal) in [
        (super::origin::OriginPlane::XY, Vec3::new(0.0, 0.0, 1.0)),
        (super::origin::OriginPlane::XZ, Vec3::new(0.0, -1.0, 0.0)),
        (super::origin::OriginPlane::YZ, Vec3::new(1.0, 0.0, 0.0)),
    ] {
        let (at, _) = view.project(plane.middle(&view)).unwrap();
        assert_eq!(super::origin::plane_at(&view, at), Some(plane));
        app.sketch_tool.as_mut().unwrap().frame = None;
        super::plane_pick::pick_plane(&mut app, &view, at);
        let frame = app.sketch_tool.as_ref().unwrap().frame.expect("the click did not pick the plane");
        assert!((frame.axis(2) - normal).length() < 1e-9, "{plane:?} faces {:?}", frame.axis(2));
        assert!(frame.t.length() < 1e-9, "{plane:?} is not through the origin");
    }
    // They stay in the positive corner when the view turns, the nearer one caught where they overlap.
    let mut camera = app.scene.camera;
    camera.yaw = -135.0;
    let behind = crate::view::View::new(camera, app.viewport_rect);
    let middle = super::origin::OriginPlane::XY.middle(&behind);
    assert!(middle.x > 0.0 && middle.y > 0.0, "the XY square moved out of the positive corner");
    // Far from the squares there is no origin plane to catch.
    assert_eq!(super::origin::plane_at(&view, app.viewport_rect.left_top() + egui::vec2(5.0, 5.0)), None);
}

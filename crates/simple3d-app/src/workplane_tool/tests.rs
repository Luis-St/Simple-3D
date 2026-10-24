use super::*;
use crate::app::tests::{headless_app, two_boxes};
use crate::app::App;
use crate::gizmo::Mode;
use crate::snap::FeatureKind;
use simple3d_core::config::Placement;
use simple3d_core::scene::GroupOp;

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-9
}

/// Put a workplane down on the face through `point` facing `normal`, as a click and Place would.
fn place(app: &mut App, point: Vec3, normal: Vec3) -> WorkplaneId {
    app.toggle_workplane_tool();
    app.workplane_tool.as_mut().unwrap().pick(CutPlane { point, normal });
    app.apply_workplane_tool();
    app.scene.settings.active_workplane.expect("Place makes the workplane active")
}

/// A workplane given straight to the settings, as a file would bring it.
fn given(app: &mut App, base: Vec3, base_rotation: Vec3, spin: f64) -> WorkplaneId {
    let mut tool = WorkplaneTool::new("Given".into(), 10.0);
    tool.plane.base = base;
    tool.plane.base_rotation = base_rotation;
    tool.plane.spin = spin;
    let id = app.scene.settings.add_workplane(tool.plane);
    app.scene.settings.active_workplane = Some(id);
    id
}

#[test]
fn a_workplane_on_a_top_face_keeps_the_world_axes_and_slides_and_turns_in_its_own() {
    let (mut app, _, _) = two_boxes("workplane-top", v(60.0, 0.0, 0.0));
    let id = place(&mut app, v(0.0, 0.0, 20.0), v(0.0, 0.0, 1.0));
    let plane = app.scene.settings.workplane(id).unwrap().clone();
    assert!(near(plane.origin(), v(0.0, 0.0, 20.0)), "centred at {:?}", plane.origin());
    let [x, y, z] = plane.axes();
    assert!(near(x, v(1.0, 0.0, 0.0)) && near(y, v(0.0, 1.0, 0.0)) && near(z, v(0.0, 0.0, 1.0)));

    let mut moved = plane.clone();
    moved.offset = [5.0, 0.0, 2.0];
    moved.spin = 90.0;
    assert!(near(moved.origin(), v(5.0, 0.0, 22.0)), "slid and lifted to {:?}", moved.origin());
    assert!((moved.axes()[0] - v(0.0, 1.0, 0.0)).length() < 1e-9, "a quarter turn did not turn X onto Y");
}

#[test]
fn on_a_side_face_the_grid_stands_upright_with_its_x_level() {
    let (mut app, _, _) = two_boxes("workplane-side", v(60.0, 0.0, 0.0));
    let id = place(&mut app, v(10.0, 0.0, 10.0), v(1.0, 0.0, 0.0));
    let [x, y, z] = app.scene.settings.workplane(id).unwrap().axes();
    assert!(x.z.abs() < 1e-9, "X is not level: {x:?}");
    assert!(near(y, v(0.0, 0.0, 1.0)), "Y does not run up the face: {y:?}");
    assert!(near(z, v(1.0, 0.0, 0.0)), "the grid does not face out of the face: {z:?}");
}

#[test]
fn placing_is_one_undo_step_and_undoing_it_falls_back_to_the_ground() {
    let (mut app, _, _) = two_boxes("workplane-undo", v(60.0, 0.0, 0.0));
    let steps = app.history.undo_len();
    place(&mut app, v(0.0, 0.0, 20.0), v(0.0, 0.0, 1.0));
    assert_eq!(app.scene.settings.workplanes.len(), 1);
    assert_eq!(app.history.undo_len(), steps + 1, "placing was not one undo step");
    app.history.undo(&mut app.scene);
    assert!(app.scene.settings.workplanes.is_empty(), "undo left the workplane");
    assert_eq!(app.scene.settings.active_workplane, None, "the ground did not take over");
}

#[test]
fn nothing_is_kept_until_a_face_is_picked_or_when_cancelled() {
    let (mut app, _, _) = two_boxes("workplane-cancel", v(60.0, 0.0, 0.0));
    app.toggle_workplane_tool();
    app.apply_workplane_tool();
    assert!(app.workplane_tool.is_some(), "Place without a face closed the tool");
    assert!(app.scene.settings.workplanes.is_empty(), "a workplane without a face was kept");
    app.workplane_tool.as_mut().unwrap().pick(CutPlane { point: v(0.0, 0.0, 20.0), normal: v(0.0, 0.0, 1.0) });
    assert!(app.grid_plane().is_some(), "the grid is not shown on the face while it is set up");
    app.close_workplane_tool();
    assert!(app.scene.settings.workplanes.is_empty() && app.grid_plane().is_none(), "cancelling kept it");
}

#[test]
fn changing_a_workplane_keeps_its_id_and_does_not_add_another() {
    let (mut app, _, _) = two_boxes("workplane-edit", v(60.0, 0.0, 0.0));
    let id = place(&mut app, v(0.0, 0.0, 20.0), v(0.0, 0.0, 1.0));
    app.edit_workplane(id);
    let tool = app.workplane_tool.as_mut().unwrap();
    tool.plane.offset[2] = 5.0;
    tool.plane.spacing = 2.5;
    app.apply_workplane_tool();
    assert_eq!(app.scene.settings.workplanes.len(), 1);
    let plane = app.scene.settings.workplane(id).expect("the workplane changed id");
    assert!(near(plane.origin(), v(0.0, 0.0, 25.0)) && plane.spacing == 2.5, "the change was not kept");
}

#[test]
fn a_new_shape_lands_at_the_workplane_middle_lying_on_it() {
    let (mut app, _, _) = two_boxes("workplane-add", v(60.0, 0.0, 0.0));
    app.reevaluate_for_test();
    place(&mut app, v(10.0, 0.0, 10.0), v(1.0, 0.0, 0.0));
    app.settings.placement = Placement::Origin;
    let id = app.insert_new(Some("box"), GroupOp::Union, Placement::Origin).unwrap();
    let node = app.scene.node(id);
    assert!(near(node.position, v(10.0, 0.0, 10.0)), "landed at {:?}", node.position);
    let own = Xform::from_pos_rot(node.position, node.rotation);
    assert!((own.axis(2) - v(1.0, 0.0, 0.0)).length() < 1e-9, "its bottom does not face the workplane");

    // A group has nothing to lay down, and back on the ground shapes keep their own turn.
    let group = app.insert_new(None, GroupOp::Union, Placement::Origin).unwrap();
    assert_eq!(app.scene.node(group).rotation, Vec3::ZERO);
    app.activate_workplane(None);
    let plain = app.insert_new(Some("box"), GroupOp::Union, Placement::Origin).unwrap();
    assert_eq!((app.scene.node(plain).position, app.scene.node(plain).rotation), (Vec3::ZERO, Vec3::ZERO));
}

#[test]
fn the_move_handles_follow_the_workplane_and_resizing_keeps_the_shapes_own_axes() {
    let (mut app, a, _) = two_boxes("workplane-gizmo", v(60.0, 0.0, 0.0));
    app.reevaluate_for_test();
    given(&mut app, Vec3::ZERO, Vec3::ZERO, 30.0);
    let turned = v(30f64.to_radians().cos(), 30f64.to_radians().sin(), 0.0);
    app.mode = Mode::Move;
    let gizmo = app.gizmo_for(a).unwrap();
    assert!((gizmo.axes[0] - turned).length() < 1e-9, "move X runs along {:?}", gizmo.axes[0]);
    app.mode = Mode::Resize;
    let gizmo = app.gizmo_for(a).unwrap();
    assert!(near(gizmo.axes[0], v(1.0, 0.0, 0.0)), "resize X left the box's own axis");
}

#[test]
fn a_snapping_pick_catches_the_workplane_grid_rather_than_the_ground() {
    let mut app = headless_app();
    given(&mut app, v(0.0, 0.0, 30.0), Vec3::ZERO, 0.0);
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let grid = app.ground_grid(&view).expect("the grid is shown");
    assert!(near(grid.plane.origin, v(0.0, 0.0, 30.0)), "the snap grid is not the workplane's");
    let s = grid.spacing;
    let crossing = v(2.0 * s, 3.0 * s, 30.0);
    let (screen, _) = view.project(crossing).unwrap();
    let point = crate::app::tests::snapped_pick(&app, &view, screen + egui::vec2(3.0, -2.0)).unwrap();
    assert_eq!(point.kind, Some(FeatureKind::GridPoint), "caught {:?}", point.kind);
    assert!(near(point.at, crossing), "landed at {:?}, not on the workplane's crossing", point.at);
}

#[test]
fn deleting_the_active_workplane_hands_back_to_the_ground() {
    let (mut app, _, _) = two_boxes("workplane-delete", v(60.0, 0.0, 0.0));
    let id = place(&mut app, v(0.0, 0.0, 20.0), v(0.0, 0.0, 1.0));
    app.delete_workplane(id);
    assert!(app.scene.settings.workplanes.is_empty());
    assert_eq!(app.workplane().map(|plane| plane.id), None);
    assert_eq!(app.work_origin(), Vec3::ZERO);
}

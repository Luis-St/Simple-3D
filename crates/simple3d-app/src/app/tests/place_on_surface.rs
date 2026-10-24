//! Putting a new shape down on a surface (issue 117).

use super::*;
use simple3d_core::config::Placement;
use simple3d_core::scene::{GroupOp, NodeId};
use simple3d_geom::Vec3;

/// A view straight down on the origin, and the screen point over `at`.
fn looking_down(app: &mut App) -> crate::view::View {
    app.scene.camera.yaw = 0.0;
    app.scene.camera.pitch = 89.0;
    app.scene.camera.target = Vec3::ZERO;
    crate::view::View::new(app.scene.camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)))
}

#[test]
pub(crate) fn a_new_shape_follows_the_pointer_onto_the_top_of_another() {
    let mut app = headless_app();
    app.add_node(Some("box"), GroupOp::Union);
    let base = app.primary().unwrap();
    app.reevaluate_for_test();
    let (_, top) = app.evaluated.node_world_bounds[&base];

    app.settings.placement = Placement::OnSurface;
    app.add_node(Some("box"), GroupOp::Union);
    let new = app.primary().unwrap();
    assert_eq!(app.placing.as_ref().map(|p| p.id), Some(new), "the new box does not follow the pointer");
    let view = looking_down(&mut app);
    let over = view.project(Vec3::new(0.0, 0.0, top.z)).unwrap().0;
    app.follow_pointer(&view, over, false);
    app.finish_placing();
    assert!(app.placing.is_none());
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.node_world_bounds[&new];
    assert!((lo.z - top.z).abs() < 1e-6, "the new box rests at {} on a top at {}", lo.z, top.z);
    assert!(((lo.x + hi.x) / 2.0).abs() < 1e-6 && ((lo.y + hi.y) / 2.0).abs() < 1e-6, "not over the point: {lo:?}");
}

#[test]
pub(crate) fn away_from_the_model_it_stands_on_the_ground_and_escape_takes_it_back() {
    let mut app = headless_app();
    app.settings.placement = Placement::OnSurface;
    let before = app.scene.depth_first().len();
    app.add_node(Some("box"), GroupOp::Union);
    let new = app.primary().unwrap();
    let view = looking_down(&mut app);
    let over = view.project(Vec3::new(40.0, 20.0, 0.0)).unwrap().0;
    app.follow_pointer(&view, over, false);
    app.reevaluate_for_test();
    let (lo, _) = app.evaluated.node_world_bounds[&new];
    assert!(lo.z.abs() < 1e-6, "it floats at {}", lo.z);
    assert_eq!(app.scene.node(new).position.x, 40.0, "not snapped to the step under the pointer");

    app.cancel_placing();
    assert_eq!(app.scene.depth_first().len(), before, "Escape left the new box in the document");
}

#[test]
pub(crate) fn with_snapping_asked_for_a_new_shape_rests_on_the_corner_it_caught() {
    // Issue 126: Ctrl held while putting a shape down puts its middle on the corner under the pointer.
    let mut app = headless_app();
    app.add_node(Some("box"), GroupOp::Union);
    let base = app.primary().unwrap();
    app.reevaluate_for_test();
    let (_, hi) = app.evaluated.node_world_bounds[&base];

    app.settings.placement = Placement::OnSurface;
    app.add_node(Some("box"), GroupOp::Union);
    let new = app.primary().unwrap();
    app.reevaluate_for_test();
    app.snap_requested = true;
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let over = view.project(hi).unwrap().0 + egui::vec2(2.0, 2.0);
    for _ in 0..3 {
        app.follow_pointer(&view, over, false);
        app.reevaluate_for_test();
    }
    let mark = app.snap_indicator.clone().expect("no snap was shown");
    assert!((mark.at - hi).length() < 1e-6, "caught {:?}, not the corner {hi:?}", mark.at);
    let (lo, top) = app.evaluated.node_world_bounds[&new];
    let middle = Vec3::new((lo.x + top.x) / 2.0, (lo.y + top.y) / 2.0, lo.z);
    assert!((middle - hi).length() < 1e-6, "rests with its bottom middle at {middle:?}, not on {hi:?}");
}

#[test]
pub(crate) fn a_new_shape_starting_at_the_3d_cursor_still_rests_where_the_pointer_is() {
    // Regression: the shape's box was measured with the cursor's offset in it, so with the cursor at
    // (10, 10, 10) every placement landed 10 mm off in each direction, and sank 10 mm into the ground.
    let mut app = headless_app();
    app.cursor = Some(Vec3::new(10.0, 10.0, 10.0));
    app.settings.placement = Placement::OnSurface;
    app.add_node(Some("box"), GroupOp::Union);
    let new = app.primary().unwrap();
    let view = looking_down(&mut app);
    let over = view.project(Vec3::new(40.0, 20.0, 0.0)).unwrap().0;
    app.follow_pointer(&view, over, false);
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.node_world_bounds[&new];
    assert!(lo.z.abs() < 1e-6, "it rests at {} instead of on the ground", lo.z);
    let middle = ((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
    assert!((middle.0 - 40.0).abs() < 1e-6 && (middle.1 - 20.0).abs() < 1e-6, "its middle is at {middle:?}");
}

/// A box to put things on, turned `turn` degrees about Z, and a new, smaller box following the
/// pointer under "On a surface"; with the view straight down and the base's evaluated top.
fn box_on_a_box(app: &mut App, turn: f64) -> (NodeId, NodeId, crate::view::View, f64) {
    app.add_node(Some("box"), GroupOp::Union);
    let base = app.primary().unwrap();
    app.scene.get_mut(base).unwrap().rotation.z = turn;
    app.reevaluate_for_test();
    let (_, top) = app.evaluated.node_world_bounds[&base];
    app.settings.placement = Placement::OnSurface;
    app.add_node(Some("box"), GroupOp::Union);
    let new = app.primary().unwrap();
    app.scene.get_mut(new).unwrap().scale = Vec3::new(0.5, 0.5, 0.5);
    let view = looking_down(app);
    (base, new, view, top.z)
}

/// The bottom middle of `id` as evaluated.
fn bottom_middle(app: &App, id: NodeId) -> Vec3 {
    let (lo, hi) = app.evaluated.node_world_bounds[&id];
    Vec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, lo.z)
}

#[test]
pub(crate) fn without_the_snap_key_a_new_shape_does_not_catch_the_face_centre() {
    // Issue 128: the face centre caught with and without Ctrl alike.
    let mut app = headless_app();
    app.scene.settings.grid_spacing = 7.0;
    let (_, _, view, top) = box_on_a_box(&mut app, 0.0);
    let over = view.project(Vec3::new(0.0, 0.0, top)).unwrap().0 + egui::vec2(3.0, -2.0);
    app.follow_pointer(&view, over, false);
    assert!(app.snap_indicator.is_none(), "caught the face centre without the snap key");
}

#[test]
pub(crate) fn with_the_snap_key_near_the_middle_of_a_face_a_new_shape_sits_on_its_centre() {
    // Issue 128: the face centre is caught off the grid step.
    let mut app = headless_app();
    app.scene.settings.grid_spacing = 7.0;
    let (_, new, view, top) = box_on_a_box(&mut app, 0.0);
    app.snap_requested = true;
    let over = view.project(Vec3::new(0.0, 0.0, top)).unwrap().0 + egui::vec2(3.0, -2.0);
    app.follow_pointer(&view, over, false);
    app.reevaluate_for_test();
    let middle = bottom_middle(&app, new);
    assert!((middle - Vec3::new(0.0, 0.0, top)).length() < 1e-6, "rests at {middle:?}, not on the face centre");
    assert!(app.snap_indicator.is_some(), "the caught centre is not shown");
}

#[test]
pub(crate) fn with_the_snap_key_near_a_corner_of_a_face_a_new_shape_puts_its_middle_on_it() {
    // Issue 128: the shape's middle on the corner, not the shape flush inside it.
    let mut app = headless_app();
    let (base, new, view, top) = box_on_a_box(&mut app, 0.0);
    let (blo, bhi) = app.evaluated.node_world_bounds[&base];
    let corner = Vec3::new(bhi.x, blo.y, top);
    app.snap_requested = true;
    let over = view.project(corner + Vec3::new(-0.5, 0.5, 0.0)).unwrap().0;
    app.follow_pointer(&view, over, false);
    app.reevaluate_for_test();
    let middle = bottom_middle(&app, new);
    assert!((middle - corner).length() < 1e-6, "rests at {middle:?}, not on the corner {corner:?}");
}

#[test]
pub(crate) fn on_a_turned_face_a_new_shape_turns_along_its_edges() {
    let mut app = headless_app();
    let (_, new, view, top) = box_on_a_box(&mut app, 30.0);
    app.snap_requested = true;
    let over = view.project(Vec3::new(0.0, 0.0, top)).unwrap().0 + egui::vec2(2.0, 2.0);
    app.follow_pointer(&view, over, false);
    let turned = app.scene.node(new).rotation.z.rem_euclid(90.0);
    assert!((turned - 30.0).abs() < 1e-6, "turned {turned} degrees, not along the face's edges");
}

#[test]
pub(crate) fn alt_leaves_the_face_snaps_out() {
    let mut app = headless_app();
    let (_, new, view, top) = box_on_a_box(&mut app, 30.0);
    app.snap_requested = true;
    let over = view.project(Vec3::new(0.0, 0.0, top)).unwrap().0 + egui::vec2(2.0, 2.0);
    app.follow_pointer(&view, over, true);
    assert_eq!(app.scene.node(new).rotation.z, 0.0, "turned along the face with Alt held");
    assert!(app.snap_indicator.is_none());
}

#[test]
pub(crate) fn with_the_snap_key_a_new_shape_puts_its_middle_on_the_edge_of_a_face() {
    let mut app = headless_app();
    let (base, new, view, top) = box_on_a_box(&mut app, 0.0);
    let (_, bhi) = app.evaluated.node_world_bounds[&base];
    app.snap_requested = true;
    let over = view.project(Vec3::new(bhi.x - 0.3, 3.0, top)).unwrap().0;
    app.follow_pointer(&view, over, false);
    app.reevaluate_for_test();
    let middle = bottom_middle(&app, new);
    assert!((middle.x - bhi.x).abs() < 1e-6 && (middle.z - top).abs() < 1e-6, "not on the edge: {middle:?}");
}

#[test]
pub(crate) fn a_caught_corner_holds_with_the_pointer_just_past_it() {
    // Issue 128: a corner caught only from inside the face, and let go of one pixel past it.
    let mut app = headless_app();
    let (base, new, view, top) = box_on_a_box(&mut app, 0.0);
    let (blo, bhi) = app.evaluated.node_world_bounds[&base];
    let corner = Vec3::new(bhi.x, blo.y, top);
    app.snap_requested = true;
    let inside = view.project(corner + Vec3::new(-0.5, 0.5, 0.0)).unwrap().0;
    app.follow_pointer(&view, inside, false);
    // 25 pixels diagonally off the face, past the 22 a corner is caught from.
    let at = view.project(corner).unwrap().0;
    let outward = view.project(corner + Vec3::new(1.0, -1.0, 0.0)).unwrap().0 - at;
    app.follow_pointer(&view, at + outward.normalized() * 25.0, false);
    app.reevaluate_for_test();
    let middle = bottom_middle(&app, new);
    assert!((middle - corner).length() < 1e-6, "let go of the corner: {middle:?}");
}

#[test]
pub(crate) fn on_a_turned_face_a_new_shape_turns_with_it_without_the_snap_key() {
    // Issue 128: turned only once caught, it spun on and off the face's edges as the pointer moved.
    let mut app = headless_app();
    let (base, new, view, top) = box_on_a_box(&mut app, 30.0);
    let (blo, bhi) = app.evaluated.node_world_bounds[&base];
    let centre = view.project(Vec3::new((blo.x + bhi.x) / 2.0, (blo.y + bhi.y) / 2.0, top)).unwrap().0;
    app.follow_pointer(&view, centre, false);
    assert!(app.snap_indicator.is_none(), "caught the centre without the snap key");
    let turned = app.scene.node(new).rotation.z.rem_euclid(90.0);
    assert!((turned - 30.0).abs() < 1e-6, "turned {turned} degrees, not along the face's edges");
}

//! Which handles a node is offered, and how they are hit.

use super::*;
use crate::view::View;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn no_resize_handle_is_offered_on_an_axis_no_parameter_governs() {
    // A torus's X and Y extents are its ring and tube diameters together, so
    // the registry withdraws those handles rather than offer one that lies.
    let f = Fixture::new("torus");
    let gizmo = f.gizmo(Mode::Resize);
    let handles = gizmo.handles(false);
    assert!(handles.contains(&Handle::ResizeFace(2, true)), "the Z handle should be offered");
    assert!(!handles.contains(&Handle::ResizeFace(0, true)), "an X handle was offered on a torus");
    assert!(!handles.contains(&Handle::ResizeFace(1, false)));
    // With only one drivable axis there is nothing for a corner to do either.
    assert!(!handles.iter().any(|h| matches!(h, Handle::ResizeCorner(_))));
}

#[test]
pub(crate) fn a_polyhedron_offers_no_resize_handles_at_all() {
    let f = Fixture::new("icosahedron");
    assert!(f.gizmo(Mode::Resize).handles(false).is_empty());
    // But it can still be moved and rotated.
    assert_eq!(f.gizmo(Mode::Move).handles(false).len(), 6);
    assert_eq!(f.gizmo(Mode::Rotate).handles(false).len(), 3);
}

#[test]
pub(crate) fn groups_get_move_and_rotate_but_not_resize() {
    // Spec section 6.2: resize handles on groups are out of scope.
    let f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Resize);
    assert!(gizmo.handles(true).is_empty());
    assert!(!gizmo.handles(false).is_empty());
    let move_gizmo = f.gizmo(Mode::Move);
    assert_eq!(move_gizmo.handles(true).len(), 6);
    assert_eq!(f.gizmo(Mode::Rotate).handles(true).len(), 3);
}

#[test]
pub(crate) fn handles_keep_a_constant_screen_size_as_the_camera_pulls_back() {
    let mut f = Fixture::new("box");
    let near_arm = {
        let gizmo = f.gizmo(Mode::Move);
        let point = gizmo.handle_point(Handle::MoveAxis(0), &f.view);
        (f.view.project(point).unwrap().0 - f.view.project(gizmo.origin).unwrap().0).length()
    };
    f.scene.camera.distance = 900.0;
    f.view = View::new(f.scene.camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)));
    f.reevaluate();
    let far_arm = {
        let gizmo = f.gizmo(Mode::Move);
        let point = gizmo.handle_point(Handle::MoveAxis(0), &f.view);
        (f.view.project(point).unwrap().0 - f.view.project(gizmo.origin).unwrap().0).length()
    };
    assert!((near_arm - far_arm).abs() < 2.0, "{near_arm} vs {far_arm} pixels");
}

#[test]
pub(crate) fn hit_testing_finds_the_handle_under_the_cursor_and_nothing_far_from_one() {
    let f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Move);
    for handle in [Handle::MoveAxis(0), Handle::MoveAxis(2), Handle::MovePlane(1)] {
        let screen = f.view.project(gizmo.handle_point(handle, &f.view)).unwrap().0;
        assert_eq!(gizmo.hit_test(&f.view, screen, false), Some(handle), "{handle:?}");
    }
    assert_eq!(gizmo.hit_test(&f.view, egui::pos2(5.0, 5.0), false), None);
}

#[test]
pub(crate) fn a_corner_wins_over_the_face_it_sits_on() {
    let f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Resize);
    let corner = Handle::ResizeCorner([true, true, true]);
    let screen = f.view.project(gizmo.handle_point(corner, &f.view)).unwrap().0;
    assert_eq!(gizmo.hit_test(&f.view, screen, false), Some(corner));
}

#[test]
pub(crate) fn a_rotate_ring_is_grabbable_along_its_whole_circumference() {
    let f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Rotate);
    let points = gizmo.ring_points(1, &f.view, 24);
    for (i, point) in points.iter().enumerate() {
        let screen = f.view.project(*point).unwrap().0;
        let hit = gizmo.hit_test(&f.view, screen, false);
        assert!(matches!(hit, Some(Handle::RotateRing(_))), "{point:?} grabbed {hit:?}");
        // The three rings genuinely cross where they meet an axis, and there
        // the nearest one legitimately wins; away from those crossings it must
        // be this ring.
        let near_axis = i % 6 <= 1 || i % 6 >= 5;
        if !near_axis {
            assert_eq!(hit, Some(Handle::RotateRing(1)), "{point:?}");
        }
    }
}

#[test]
pub(crate) fn the_handle_frame_follows_the_nodes_own_rotation() {
    // There is one frame now, the node's own: the switch to the world's axes
    // went with the rail button that worked it (issue 100).
    let mut f = Fixture::new("box");
    f.scene.get_mut(f.node).unwrap().rotation = Vec3::new(0.0, 0.0, 90.0);
    f.reevaluate();
    let gizmo = Gizmo::build(&f.scene, &f.evaluated, f.node, Mode::Move).unwrap();
    // The node's local X now points along world +Y, and so does its handle.
    assert!((gizmo.axes[0] - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-9, "{:?}", gizmo.axes[0]);
}

#[test]
pub(crate) fn the_gizmo_is_not_offered_for_the_scene_root() {
    let f = Fixture::new("box");
    assert!(Gizmo::build(&f.scene, &f.evaluated, f.scene.root(), Mode::Move).is_none());
}

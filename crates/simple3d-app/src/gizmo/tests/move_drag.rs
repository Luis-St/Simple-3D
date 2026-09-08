//! Move drags: which axes they touch, and what escape puts back.

use super::*;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_move_axis_drag_moves_only_that_axis() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Move);
    let handle = Handle::MoveAxis(0);
    let start = gizmo.handle_point(handle, &f.view);
    drag_to(&mut f, handle, start + Vec3::new(30.0, 0.0, 0.0), Mods { free: true, ..Default::default() }, 10.0);
    let position = f.scene.node(f.node).position;
    assert!((position.x - 30.0).abs() < 0.2, "{position:?}");
    assert!(position.y.abs() < 1e-6 && position.z.abs() < 1e-6, "other axes moved: {position:?}");
}

#[test]
pub(crate) fn a_move_drag_snaps_to_the_increment_and_a_modifier_frees_it() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Move);
    let handle = Handle::MoveAxis(0);
    let start = gizmo.handle_point(handle, &f.view);
    let target = start + Vec3::new(23.0, 0.0, 0.0);

    drag_to(&mut f, handle, target, Mods::default(), 10.0);
    assert!((f.scene.node(f.node).position.x - 20.0).abs() < 1e-9, "snapped to {:?}", f.scene.node(f.node).position);

    let mut fresh = Fixture::new("box");
    drag_to(&mut fresh, handle, target, Mods { free: true, ..Default::default() }, 10.0);
    let free = fresh.scene.node(fresh.node).position.x;
    assert!((free - 23.0).abs() < 0.2 && (free - 20.0).abs() > 1.0, "free drag snapped anyway: {free}");

    let mut coarse = Fixture::new("box");
    drag_to(&mut coarse, handle, start + Vec3::new(63.0, 0.0, 0.0), Mods { coarse: true, ..Default::default() }, 10.0);
    assert!((coarse.scene.node(coarse.node).position.x - 100.0).abs() < 1e-9, "coarse snap");
}

#[test]
pub(crate) fn a_plane_handle_moves_two_axes_and_leaves_the_third() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Move);
    let handle = Handle::MovePlane(2); // the XY plane
    let start = gizmo.handle_point(handle, &f.view);
    drag_to(&mut f, handle, start + Vec3::new(20.0, 30.0, 0.0), Mods { free: true, ..Default::default() }, 10.0);
    let p = f.scene.node(f.node).position;
    assert!((p.x - 20.0).abs() < 0.3 && (p.y - 30.0).abs() < 0.3, "{p:?}");
    assert!(p.z.abs() < 1e-6, "the plane's normal axis moved: {p:?}");
}

#[test]
pub(crate) fn escape_restores_the_pre_drag_state_exactly() {
    // Spec acceptance criterion 23.
    let mut f = Fixture::new("box");
    let before_position = f.scene.node(f.node).position;
    let before_params = f.scene.node(f.node).params().cloned().unwrap();
    let gizmo = f.gizmo(Mode::Move);
    let handle = Handle::MoveAxis(1);
    let start = gizmo.handle_point(handle, &f.view);
    let drag = drag_to(&mut f, handle, start + Vec3::new(0.0, 37.0, 0.0), Mods::default(), 10.0);
    assert_ne!(f.scene.node(f.node).position, before_position);
    drag.cancel(&mut f.scene);
    assert_eq!(f.scene.node(f.node).position, before_position);
    assert_eq!(f.scene.node(f.node).params().unwrap(), &before_params);
}

#[test]
pub(crate) fn a_move_drag_on_a_rotated_child_writes_parent_frame_coordinates() {
    let mut f = Fixture::new("box");
    let root = f.scene.root();
    let group = f.scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
    f.scene.get_mut(group).unwrap().rotation = Vec3::new(0.0, 0.0, 90.0);
    f.scene.reparent(f.node, group, 0).unwrap();
    f.reevaluate();

    let gizmo = f.gizmo(Mode::Move);
    // Drag along the child's local X, which the group has turned into world +Y.
    let handle = Handle::MoveAxis(0);
    let start = gizmo.handle_point(handle, &f.view);
    drag_to(&mut f, handle, start + Vec3::new(0.0, 30.0, 0.0), Mods { free: true, ..Default::default() }, 10.0);
    let position = f.scene.node(f.node).position;
    // Stored in the parent's frame, so it reads as +30 on X, not on Y.
    assert!((position.x - 30.0).abs() < 0.3, "{position:?}");
    assert!(position.y.abs() < 0.3, "{position:?}");
    // And the geometry really moved along world +Y.
    let (lo, hi) = f.world_bounds();
    assert!(((lo.y + hi.y) / 2.0 - 30.0).abs() < 0.3, "{lo:?} {hi:?}");
}

//! Resizing by a face or a corner, and the modifiers that change it.

use super::*;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn dragging_the_right_face_changes_the_width_and_leaves_the_left_face() {
    // Spec acceptance criterion 24, the central one for this module.
    let mut f = Fixture::new("box");
    let (lo_before, hi_before) = f.world_bounds();
    assert!((hi_before.x - lo_before.x - 20.0).abs() < 1e-9);

    let gizmo = f.gizmo(Mode::Resize);
    let handle = Handle::ResizeFace(0, true);
    let start = gizmo.handle_point(handle, &f.view);
    drag_to(&mut f, handle, start + Vec3::new(10.0, 0.0, 0.0), Mods::default(), 10.0);

    assert!((f.param("width") - 30.0).abs() < 1e-9, "width is {}", f.param("width"));
    let (lo_after, hi_after) = f.world_bounds();
    assert!((lo_after.x - lo_before.x).abs() < 1e-9, "the left face moved: {} -> {}", lo_before.x, lo_after.x);
    assert!((hi_after.x - hi_before.x - 10.0).abs() < 1e-9, "the right face did not follow");
    // Nothing else changed.
    assert!((hi_after.y - lo_after.y - 20.0).abs() < 1e-9);
    assert!((hi_after.z - lo_after.z - 20.0).abs() < 1e-9);
}

#[test]
pub(crate) fn dragging_the_left_face_leaves_the_right_face() {
    let mut f = Fixture::new("box");
    let (_, hi_before) = f.world_bounds();
    let gizmo = f.gizmo(Mode::Resize);
    let handle = Handle::ResizeFace(0, false);
    let start = gizmo.handle_point(handle, &f.view);
    drag_to(&mut f, handle, start + Vec3::new(-10.0, 0.0, 0.0), Mods::default(), 10.0);
    assert!((f.param("width") - 30.0).abs() < 1e-9, "width is {}", f.param("width"));
    let (lo_after, hi_after) = f.world_bounds();
    assert!((hi_after.x - hi_before.x).abs() < 1e-9, "the right face moved");
    assert!((hi_after.x - lo_after.x - 30.0).abs() < 1e-9);
}

#[test]
pub(crate) fn the_symmetry_modifier_grows_about_the_centre() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Resize);
    let handle = Handle::ResizeFace(0, true);
    let start = gizmo.handle_point(handle, &f.view);
    drag_to(&mut f, handle, start + Vec3::new(10.0, 0.0, 0.0), Mods { symmetric: true, ..Default::default() }, 10.0);
    assert!((f.param("width") - 40.0).abs() < 1e-9, "width is {}", f.param("width"));
    let (lo, hi) = f.world_bounds();
    assert!(((lo.x + hi.x) / 2.0).abs() < 1e-9, "the centre moved: {lo:?} {hi:?}");
    assert_eq!(f.scene.node(f.node).position, Vec3::ZERO);
}

#[test]
pub(crate) fn a_corner_drag_with_the_proportions_modifier_keeps_the_ratio() {
    // Spec acceptance criterion 25.
    let mut f = Fixture::new("plate");
    let ratio_before = f.param("width") / f.param("depth");
    let gizmo = f.gizmo(Mode::Resize);
    let handle = Handle::ResizeCorner([true, true, true]);
    let start = gizmo.handle_point(handle, &f.view);
    drag_to(
        &mut f,
        handle,
        start + Vec3::new(15.0, 8.0, 0.0),
        Mods { symmetric: true, free: true, ..Default::default() },
        10.0,
    );
    let ratio_after = f.param("width") / f.param("depth");
    assert!((ratio_after - ratio_before).abs() < 1e-6, "{ratio_before} -> {ratio_after}");
    assert!(f.param("width") > 40.0, "the corner drag did nothing: {}", f.param("width"));
    // The third dimension scaled by the same ratio too.
    let scale = f.param("width") / 40.0;
    assert!((f.param("thickness") - 4.0 * scale).abs() < 1e-6);
}

#[test]
pub(crate) fn a_dimension_cannot_be_dragged_to_zero_or_negative() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Resize);
    let handle = Handle::ResizeFace(0, true);
    let start = gizmo.handle_point(handle, &f.view);
    drag_to(&mut f, handle, start + Vec3::new(-500.0, 0.0, 0.0), Mods { free: true, ..Default::default() }, 10.0);
    assert!(f.param("width") > 0.0, "width went to {}", f.param("width"));
    let (lo, hi) = f.world_bounds();
    assert!(hi.x >= lo.x, "the shape inverted");
}

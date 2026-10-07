//! What stays put while sizing (issue 120).

use super::*;
use simple3d_core::config::SizeAnchor;
use simple3d_core::scene::Anchor;

/// Pull `handle` by `by` in `mode` with `anchor` holding on every axis.
fn pull(f: &mut Fixture, mode: Mode, handle: Handle, by: Vec3, anchor: SizeAnchor) {
    let mut gizmo = f.gizmo(mode);
    gizmo.anchor = anchor;
    let from = f.view.project(gizmo.handle_point(handle, &f.view)).unwrap().0;
    let to = f.view.project(gizmo.handle_point(handle, &f.view) + by).unwrap().0;
    let mut drag = Drag::begin(&f.scene, &gizmo, f.node, handle, &f.view, from).unwrap();
    drag.update(&mut f.scene, &f.view, to, Mods::default(), 10.0, 15.0, Unit::Millimetre);
    f.reevaluate();
}

#[test]
pub(crate) fn the_opposite_side_stays_whichever_face_is_pulled() {
    for mode in [Mode::Resize, Mode::Scale] {
        for positive in [true, false] {
            let mut f = Fixture::new("box");
            let (lo, hi) = f.world_bounds();
            let by = Vec3::new(if positive { 10.0 } else { -10.0 }, 0.0, 0.0);
            pull(&mut f, mode, Handle::ResizeFace(0, positive), by, SizeAnchor::Opposite);
            let (lo_after, hi_after) = f.world_bounds();
            let (held, held_after) = if positive { (lo.x, lo_after.x) } else { (hi.x, hi_after.x) };
            assert!((held_after - held).abs() < 1e-9, "{mode:?} {positive}: the far side moved to {held_after}");
            assert!(
                (hi_after.x - lo_after.x - 30.0).abs() < 1e-9,
                "{mode:?} {positive}: width {}",
                hi_after.x - lo_after.x
            );
        }
    }
}

#[test]
pub(crate) fn a_centre_anchor_grows_both_sides_without_the_modifier() {
    let mut f = Fixture::new("box");
    pull(&mut f, Mode::Resize, Handle::ResizeFace(0, true), Vec3::new(10.0, 0.0, 0.0), SizeAnchor::Centre);
    let (lo, hi) = f.world_bounds();
    assert!((hi.x - lo.x - 40.0).abs() < 1e-9, "width {}", hi.x - lo.x);
    assert!(((lo.x + hi.x) / 2.0).abs() < 1e-9, "the centre moved: {lo:?} {hi:?}");
}

#[test]
pub(crate) fn the_opposite_side_stays_for_an_object_standing_on_its_base() {
    // The origin is at the bottom, so growing about it is not growing about the middle.
    let mut f = Fixture::new("box");
    f.scene.get_mut(f.node).unwrap().anchor = Anchor::Base;
    f.reevaluate();
    let (lo, hi) = f.world_bounds();
    pull(&mut f, Mode::Scale, Handle::ResizeFace(2, true), Vec3::new(0.0, 0.0, 10.0), SizeAnchor::Opposite);
    let (lo_after, hi_after) = f.world_bounds();
    assert!((lo_after.z - lo.z).abs() < 1e-9, "the base moved: {} -> {}", lo.z, lo_after.z);
    assert!((hi_after.z - hi.z - 10.0).abs() < 1e-9, "the top took {}", hi_after.z - hi.z);
}

#[test]
pub(crate) fn the_arrow_keys_hold_the_centre() {
    let mut f = Fixture::new("box");
    f.scene.get_mut(f.node).unwrap().anchor = Anchor::Base;
    f.reevaluate();
    let (lo, hi) = f.world_bounds();
    let gizmo = f.gizmo(Mode::Resize);
    let (axis, _) = nudge_axis(&gizmo, &f.view, simple3d_core::keymap::Command::NudgeRight).unwrap();
    let step = nudge_step(&gizmo, &f.view, simple3d_core::keymap::Command::NudgeRight, 5.0, 15.0).unwrap();
    step.apply(&gizmo, &mut f.scene, f.node);
    f.reevaluate();
    let (lo_after, hi_after) = f.world_bounds();
    let (centre, centre_after) = ((lo.get(axis) + hi.get(axis)) / 2.0, (lo_after.get(axis) + hi_after.get(axis)) / 2.0);
    assert!((centre_after - centre).abs() < 1e-9, "the centre moved on axis {axis}: {centre} -> {centre_after}");
}

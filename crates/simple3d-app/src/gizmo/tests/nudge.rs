//! Nudging by key, and the undo run a held key makes.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::undo::History;

#[test]
pub(crate) fn the_screen_aligned_axes_are_a_permutation_matched_to_the_view() {
    let f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Move);
    let [horizontal, vertical, third] = screen_aligned_axes(&gizmo, &f.view);
    let mut sorted = [horizontal, vertical, third];
    sorted.sort_unstable();
    assert_eq!(sorted, [0, 1, 2], "not a permutation");
    // From the default view, Z is the most vertical axis.
    assert_eq!(vertical, 2, "expected Z to be the screen-vertical axis");
    let (right, _) = f.view.basis();
    assert!(
        gizmo.axes[horizontal].dot(right).abs() > gizmo.axes[third].dot(right).abs(),
        "the horizontal axis is not the most horizontal one"
    );
}

#[test]
pub(crate) fn a_nudge_left_really_goes_left() {
    let f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Move);
    let [horizontal, vertical, _] = screen_aligned_axes(&gizmo, &f.view);
    let (right, up) = f.view.basis();
    let h_sign = axis_screen_sign(&gizmo, &f.view, horizontal, false);
    assert!((gizmo.axes[horizontal] * h_sign).dot(right) > 0.0);
    let v_sign = axis_screen_sign(&gizmo, &f.view, vertical, true);
    assert!((gizmo.axes[vertical] * v_sign).dot(up) > 0.0);
}

/// Spec acceptance criterion 26: arrow-key nudges repeat when held, step by the snap increment,
/// and a whole run is one undo step. Tested through `apply_nudge`, the real path.
#[test]
pub(crate) fn a_held_nudge_run_steps_by_the_snap_and_undoes_in_one() {
    const SNAP: f64 = 2.5;
    const PRESSES: usize = 12;

    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Move);
    let start = f.scene.node(f.node).position;

    let mut history = History::new();
    // A step before the run, so "one step" differs from "stack emptied".
    history.record(&f.scene, "Before", None);

    // Holding the key: each repeat goes through the full path.
    for _ in 0..PRESSES {
        let step = apply_nudge(&mut history, &mut f.scene, &gizmo, &f.view, f.node, Command::NudgeRight, SNAP, 15.0)
            .expect("a nudge command");
        let Nudge::Move { axis, world_delta } = step else { panic!("move mode gave {step:?}") };
        assert!(
            (world_delta.length() - SNAP).abs() < 1e-9,
            "one press moved {} mm, not the {SNAP} mm snap increment",
            world_delta.length()
        );
        assert!((world_delta * (1.0 / SNAP) - gizmo.axes[axis]).length() < 1e-9, "the step left its axis");
    }

    // Every press moved.
    let after = f.scene.node(f.node).position;
    let travelled = (gizmo.parent.point(after) - gizmo.parent.point(start)).length();
    assert!(
        (travelled - SNAP * PRESSES as f64).abs() < 1e-6,
        "{PRESSES} presses travelled {travelled} mm, expected {}",
        SNAP * PRESSES as f64
    );

    // ...and all of it undoes at once.
    assert_eq!(history.undo(&mut f.scene).as_deref(), Some("Nudge"));
    assert_eq!(f.scene.node(f.node).position, start, "one undo did not restore the pre-run position");
    assert_eq!(history.undo_label(), Some("Before"), "the run left more than one undo step behind");

    // Redo restores the whole run at once.
    assert_eq!(history.redo(&mut f.scene).as_deref(), Some("Nudge"));
    assert_eq!(f.scene.node(f.node).position, after);
}

/// The coalesce key: direction changes stay one gesture, a different node or mode starts a new step.
#[test]
pub(crate) fn a_nudge_coalesces_across_directions_but_not_across_nodes_or_modes() {
    let mut f = Fixture::new("box");
    let root = f.scene.root();
    let other = f.scene.add_primitive("box", root, 1).unwrap();
    f.reevaluate();

    let key = nudge_coalesce_key(f.node, Mode::Move);
    assert_eq!(key, nudge_coalesce_key(f.node, Mode::Move), "the key is not stable across presses");
    assert_ne!(key, nudge_coalesce_key(other, Mode::Move), "two nodes share a coalesce key");
    assert_ne!(key, nudge_coalesce_key(f.node, Mode::Rotate), "two modes share a coalesce key");

    let move_gizmo = f.gizmo(Mode::Move);
    let rotate_gizmo = f.gizmo(Mode::Rotate);
    let other_gizmo = Gizmo::build(&f.scene, &f.evaluated, other, Mode::Move).unwrap();

    // Left, right, left: one gesture.
    let mut history = History::new();
    for command in [Command::NudgeLeft, Command::NudgeRight, Command::NudgeLeft] {
        apply_nudge(&mut history, &mut f.scene, &move_gizmo, &f.view, f.node, command, 1.0, 15.0).unwrap();
    }
    // Switching mode, and switching node, each start a new step.
    apply_nudge(&mut history, &mut f.scene, &rotate_gizmo, &f.view, f.node, Command::NudgeUp, 1.0, 15.0).unwrap();
    apply_nudge(&mut history, &mut f.scene, &other_gizmo, &f.view, other, Command::NudgeUp, 1.0, 15.0).unwrap();

    let mut steps = 0;
    while history.undo(&mut f.scene).is_some() {
        steps += 1;
    }
    assert_eq!(steps, 3, "expected the three same-key presses to merge and nothing else to");
}

/// Criterion 26's step rule for rotate (rotation snap) and resize (scene step).
#[test]
pub(crate) fn a_nudge_steps_by_the_snap_in_rotate_and_resize_too() {
    let mut f = Fixture::new("box");

    let gizmo = f.gizmo(Mode::Rotate);
    let step = nudge_step(&gizmo, &f.view, Command::NudgeUp, 2.5, 15.0).expect("a nudge command");
    let Nudge::Rotate { axis, degrees } = step else { panic!("rotate mode gave {step:?}") };
    assert_eq!(degrees.abs(), 15.0, "rotate did not step by the rotation snap");
    let before = f.scene.node(f.node).rotation.get(axis);
    step.apply(&gizmo, &mut f.scene, f.node);
    assert!((f.scene.node(f.node).rotation.get(axis) - (before + degrees)).abs() < 1e-9);

    // A fresh box, since the rotation above would skew the world bounds.
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Resize);
    let (axis, _) = nudge_axis(&gizmo, &f.view, Command::NudgeRight).unwrap();
    let extent = gizmo.local_hi.get(axis) - gizmo.local_lo.get(axis);
    let step = nudge_step(&gizmo, &f.view, Command::NudgeRight, 2.5, 15.0).expect("a nudge command");
    let Nudge::Resize { extent: target, driver, .. } = step else { panic!("resize mode gave {step:?}") };
    assert!((target - (extent + 2.5)).abs() < 1e-9, "resize did not step by the scene step");
    let before = f.param(driver.param);
    step.apply(&gizmo, &mut f.scene, f.node);
    f.reevaluate();
    // The box is unrotated, so local axes are world axes.
    let (lo, hi) = f.world_bounds();
    let measured = hi.get(axis) - lo.get(axis);
    assert!((measured - (extent + 2.5)).abs() < 1e-6, "the measured extent {measured} did not follow the nudge");
    // Criterion 24 holds for keys too: a dimension changed, not a scale.
    assert!((f.param(driver.param) - (before + 2.5 / driver.factor)).abs() < 1e-9, "resizing wrote no dimension");
}

/// A resize nudge on an ungoverned axis reports itself and changes nothing.
#[test]
pub(crate) fn a_resize_nudge_on_an_ungoverned_axis_says_so() {
    let mut f = Fixture::new("sphere");
    let gizmo = f.gizmo(Mode::Resize);
    // Find an axis with no driver, if this primitive has one.
    let ungoverned = (0..3).find(|&a| gizmo.drivers[a].is_none());
    let Some(axis) = ungoverned else { return };
    let before = f.scene.node(f.node).params().unwrap().clone();
    Nudge::NoDimension { axis }.apply(&gizmo, &mut f.scene, f.node);
    assert_eq!(f.scene.node(f.node).params().unwrap(), &before, "an ungoverned axis still wrote something");
}

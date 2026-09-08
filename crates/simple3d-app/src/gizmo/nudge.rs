//! Nudging by keyboard: the axis, the step, and the undo run it joins.

use super::*;
use crate::view::View;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::{AxisDriver, ParamValue};
use simple3d_core::scene::{Node, NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_core::unit::wrap_degrees;
use simple3d_geom::Vec3;

/// What one press of a nudge key does, in the terms the caller has to write back
/// (spec section 6.2, acceptance criterion 26).
///
/// This lives here rather than in `App::nudge` so the arithmetic -- which axis,
/// which direction, and how far -- can be asserted from a test. `App` needs a
/// live `egui::Context` to construct, which makes anything inside it unreachable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nudge {
    /// Move the node this far in world space.
    Move { axis: usize, world_delta: Vec3 },
    /// Add this many degrees to the node's rotation about `axis`.
    Rotate { axis: usize, degrees: f64 },
    /// Rewrite `driver`'s parameter so the extent along `axis` becomes `extent`.
    Resize { axis: usize, driver: AxisDriver, extent: f64 },
    /// Set the node's own scale factor on `axis` to `factor`.
    Scale { axis: usize, factor: f64 },
    /// A resize nudge on an axis no parameter governs: nothing to write, and the
    /// caller says so rather than silently doing nothing.
    NoDimension { axis: usize },
}

impl Nudge {
    /// Write the step into the scene. `gizmo` must be the one it was computed
    /// from -- the move case needs its parent frame to turn a world delta back
    /// into `Node::position`, which lives in the parent's coordinates.
    ///
    /// Caller records the undo snapshot first, with `nudge_coalesce_key`.
    pub fn apply(self, gizmo: &Gizmo, scene: &mut Scene, id: NodeId) {
        match self {
            Nudge::Move { world_delta, .. } => {
                let Some(node) = scene.get(id) else { return };
                let target = gizmo.parent.point(node.position) + world_delta;
                let local = gizmo.parent.inverse().point(target);
                if let Some(node) = scene.get_mut(id) {
                    node.position = local;
                }
            }
            Nudge::Rotate { axis, degrees } => {
                if let Some(node) = scene.get_mut(id) {
                    let mut rotation = node.rotation;
                    // Brought back into one turn, the way the ring and the
                    // rotation field both do it (issue 84): holding an arrow key
                    // down would otherwise wind the number up past 360 and leave
                    // it there.
                    let turned = wrap_degrees(get_axis(rotation, axis) + degrees);
                    set_axis(&mut rotation, axis, turned);
                    node.rotation = rotation;
                }
            }
            Nudge::Resize { driver, extent, .. } => {
                if let Some(params) = scene.get_mut(id).and_then(|n| n.params_mut()) {
                    params.insert(driver.param.to_string(), ParamValue::Length(extent / driver.factor));
                }
            }
            Nudge::Scale { axis, factor } => {
                if let Some(node) = scene.get_mut(id) {
                    let mut scale = Node::sane_scale(node.scale);
                    set_axis(&mut scale, axis, factor);
                    node.scale = scale;
                }
            }
            // Nothing this axis can be resized by. The caller says so; silently
            // doing nothing would look like a dropped keypress.
            Nudge::NoDimension { .. } => {}
        }
    }
}

/// The signed handle-frame axis a nudge command acts on: the axis index and
/// `±1`, already corrected against the view so "left" really goes left.
pub fn nudge_axis(gizmo: &Gizmo, view: &View, command: Command) -> Option<(usize, f64)> {
    let [horizontal, vertical, third] = screen_aligned_axes(gizmo, view);
    let (axis, mut sign) = match command {
        Command::NudgeLeft => (horizontal, -1.0),
        Command::NudgeRight => (horizontal, 1.0),
        Command::NudgeDown => (vertical, -1.0),
        Command::NudgeUp => (vertical, 1.0),
        Command::NudgeToward => (third, -1.0),
        Command::NudgeAway => (third, 1.0),
        _ => return None,
    };
    // The third axis has no screen direction to match, so it is left alone.
    let vertical_key = matches!(command, Command::NudgeUp | Command::NudgeDown);
    if vertical_key || matches!(command, Command::NudgeLeft | Command::NudgeRight) {
        sign *= axis_screen_sign(gizmo, view, axis, vertical_key);
    }
    Some((axis, sign))
}

/// One press of a nudge key. `move_snap` is the scene step, which governs both
/// the move step and the resize step; `rotate_snap_deg` governs rotation.
pub fn nudge_step(gizmo: &Gizmo, view: &View, command: Command, move_snap: f64, rotate_snap_deg: f64) -> Option<Nudge> {
    let (axis, sign) = nudge_axis(gizmo, view, command)?;
    Some(match gizmo.mode {
        Mode::Move => Nudge::Move { axis, world_delta: gizmo.axes[axis] * (move_snap * sign) },
        Mode::Rotate => Nudge::Rotate { axis, degrees: rotate_snap_deg * sign },
        Mode::Resize => match gizmo.drivers[axis] {
            Some(driver) => {
                let extent = get_axis(gizmo.local_hi, axis) - get_axis(gizmo.local_lo, axis);
                Nudge::Resize { axis, driver, extent: (extent + move_snap * sign).max(MIN_EXTENT) }
            }
            None => Nudge::NoDimension { axis },
        },
        // A scale nudge moves the face by the step, the same distance a resize
        // nudge would -- expressed as the factor that produces it, since that is
        // what a scale writes.
        Mode::Scale => {
            let local = get_axis(gizmo.local_hi, axis) - get_axis(gizmo.local_lo, axis);
            let world = local * gizmo.axis_scale[axis];
            if world <= MIN_EXTENT {
                Nudge::NoDimension { axis }
            } else {
                let factor = ((world + move_snap * sign).max(MIN_EXTENT)) / world;
                let grown = (get_axis(gizmo.own_scale, axis) * factor).max(Node::MIN_SCALE);
                Nudge::Scale { axis, factor: grown }
            }
        }
    })
}

/// The undo-coalescing key for a nudge. Every press within the coalescing window
/// that carries the same key extends one undo step, so holding an arrow key down
/// undoes in one (acceptance criterion 26). It deliberately does *not* mention
/// the direction: a run of left presses followed by right presses is still one
/// gesture, but nudging a different node, or in a different mode, is not.
pub fn nudge_coalesce_key(id: NodeId, mode: Mode) -> String {
    format!("nudge:{id}:{mode:?}")
}

/// One press of a nudge key, all the way through: work out the step, open or
/// extend the undo run it belongs to, and write it into the scene.
///
/// This is `App::nudge` minus the parts that need a live `egui::Context` -- the
/// selection, the status line and the field cache. Keeping the undo record here
/// rather than at the call site is what lets criterion 26's "the whole repeat run
/// is a single undo step" be asserted against the code that actually runs.
/// Returns the step taken, so the caller can report an axis with no dimension.
pub fn apply_nudge(
    history: &mut History,
    scene: &mut Scene,
    gizmo: &Gizmo,
    view: &View,
    id: NodeId,
    command: Command,
    move_snap: f64,
    rotate_snap_deg: f64,
) -> Option<Nudge> {
    let step = nudge_step(gizmo, view, command, move_snap, rotate_snap_deg)?;
    // Record before mutating, under a key stable across the whole held run.
    history.record(scene, "Nudge", Some(&nudge_coalesce_key(id, gizmo.mode)));
    step.apply(gizmo, scene, id);
    Some(step)
}

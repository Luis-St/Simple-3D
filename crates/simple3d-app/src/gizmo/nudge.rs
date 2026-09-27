//! Nudging by keyboard: the axis, the step, and the undo run it joins.

use super::*;
use crate::view::View;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::{AxisDriver, ParamValue};
use simple3d_core::scene::{Node, NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_core::unit::wrap_degrees;
use simple3d_geom::Vec3;

/// What one nudge press does (spec section 6.2, acceptance criterion 26). Separate from
/// `App::nudge` so tests can check it without a live `egui::Context`.
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
    /// A resize on an axis no parameter governs; the caller reports it.
    NoDimension { axis: usize },
}

impl Nudge {
    /// Write the step into the scene. `gizmo` must be the one it was computed from, whose parent frame
    /// converts a world move into `Node::position`. The caller records undo first.
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
                    // Wrapped into one turn (issue 84), so holding a key does not wind past 360.
                    let turned = wrap_degrees(get_axis(rotation, axis) + degrees);
                    set_axis(&mut rotation, axis, turned);
                    node.position = gizmo.position_keeping_pivot(rotation, node.scale);
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
            // Nothing to resize by; the caller reports it rather than seeming to drop the key.
            Nudge::NoDimension { .. } => {}
        }
    }
}

/// The signed handle-frame axis a nudge acts on, corrected against the view so "left" goes left.
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
    // The third axis has no screen direction to match.
    let vertical_key = matches!(command, Command::NudgeUp | Command::NudgeDown);
    if vertical_key || matches!(command, Command::NudgeLeft | Command::NudgeRight) {
        sign *= axis_screen_sign(gizmo, view, axis, vertical_key);
    }
    Some((axis, sign))
}

/// One nudge press. `move_snap` governs move and resize steps; `rotate_snap_deg` rotation.
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
        // A scale nudge moves the face by one step, expressed as the factor that achieves it.
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

/// The undo-coalescing key for a nudge, so a held key is one undo step (acceptance criterion 26).
/// Direction is deliberately omitted; node and mode are included.
pub fn nudge_coalesce_key(id: NodeId, mode: Mode) -> String {
    format!("nudge:{id}:{mode:?}")
}

/// One nudge press end to end: compute, record undo, apply. `App::nudge` minus the context-bound
/// parts, so criterion 26 can be tested against the real code. Returns the step taken.
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
    // Record before mutating, under a key stable across the held run.
    history.record(scene, "Nudge", Some(&nudge_coalesce_key(id, gizmo.mode)));
    step.apply(gizmo, scene, id);
    Some(step)
}

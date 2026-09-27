//! Dragging a field to change its number.

use super::*;
use crate::app::App;
use crate::ui::{self};
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::{Node, NodeId};
use simple3d_core::unit::{wrap_degrees, Unit};

/// One frame of a scrub on a dimension; only `started` snapshots undo, the rest `touch`, so a drag
/// is one undo step.
pub(crate) fn scrub_param(
    app: &mut App,
    targets: &[NodeId],
    param: &simple3d_core::primitive::ParamSpec,
    kind: ParamKind,
    unit: Unit,
    delta: f64,
    started: bool,
) {
    if started {
        app.edit(&format!("Scrub {}", param.label), None);
    }
    // Counts carry the fraction between frames, since re-reading the rounded value either stalled or
    // overshot.
    let whole = matches!(kind, ParamKind::Count { .. });
    let carried = if whole { app.scrub.carry } else { 0.0 };
    let mut owed = carried;
    for target in targets {
        let current = ui::param_number(param_value(app, *target, param.key, param.default));
        let shown = match kind {
            ParamKind::Length { .. } => unit.from_mm(current),
            _ => current,
        };
        let wanted = shown + delta + carried;
        let next = ui::value_from_display(kind, unit, wanted);
        set_param(app, *target, param.key, next);
        apply_lock(app, *target, param.lock_group, param.key, next);
        // Measured against what the field took, so a count at its limit does not build up debt before
        // the drag can turn round.
        if whole {
            owed = (wanted - ui::param_number(next)).clamp(-1.0, 1.0);
        }
    }
    if whole {
        app.scrub.carry = owed;
    }
    app.touch();
    app.fields.clear();
}

/// One frame of a scrub on a position (millimetres) or rotation (degrees).
pub(crate) fn scrub_transform(
    app: &mut App,
    targets: &[NodeId],
    axis: usize,
    delta: f64,
    rotation: bool,
    started: bool,
) {
    if started {
        app.edit(if rotation { "Scrub rotation" } else { "Scrub position" }, None);
    }
    for target in targets {
        let Some(node) = app.scene.get_mut(*target) else { continue };
        let mut v = if rotation { node.rotation } else { node.position };
        let next = v.get(axis) + delta;
        v.set(axis, if rotation { wrap_degrees(next) } else { next });
        if rotation {
            node.rotation = v;
        } else {
            node.position = v;
        }
    }
    app.touch();
    app.fields.clear();
}

/// One frame of a scrub on a scale field, stepping by 0.05, visible on any shape.
pub(crate) fn scrub_scale(app: &mut App, targets: &[NodeId], axis: usize, delta: f64, started: bool) {
    if started {
        app.edit("Scrub scale", None);
    }
    for target in targets {
        let Some(node) = app.scene.get_mut(*target) else { continue };
        let mut s = Node::sane_scale(node.scale);
        let next = (s.get(axis) + delta).max(Node::MIN_SCALE);
        s.set(axis, next);
        node.scale = s;
    }
    app.touch();
    app.fields.clear();
}

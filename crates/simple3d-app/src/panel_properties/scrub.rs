//! Dragging a label to change the number beside it.

use super::*;
use crate::app::App;
use crate::ui::{self};
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::{Node, NodeId};
use simple3d_core::unit::{wrap_degrees, Unit};

/// One frame of a scrub on a dimension.
///
/// `started` is the only frame that takes an undo snapshot. Every frame after
/// it goes through `touch`, which re-evaluates without recording -- so a drag
/// across forty pixels is one step to undo, not forty.
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
    // A count is stored whole, and every frame reads the stored value back before
    // adding this frame's movement to it -- so the fraction of a step each frame
    // is worth was rounded away rather than added up, and the field either never
    // moved or ran away from the pointer. The fraction is carried instead.
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
        // Measured against what the field actually took rather than against the
        // rounding alone, so a count sitting on its own limit does not build up a
        // debt that has to be paid off before the drag can turn round.
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

/// One frame of a scrub on a position (millimetres) or a rotation (degrees).
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
        let next = crate::gizmo::get_axis(v, axis) + delta;
        set_component(&mut v, axis, if rotation { wrap_degrees(next) } else { next });
        if rotation {
            node.rotation = v;
        } else {
            node.position = v;
        }
    }
    app.touch();
    app.fields.clear();
}

/// One frame of a scrub on a scale field. The grip steps by 0.05 -- a twentieth
/// is a visible change on any shape, where a millimetre-sized step would be
/// nothing on a factor.
pub(crate) fn scrub_scale(app: &mut App, targets: &[NodeId], axis: usize, delta: f64, started: bool) {
    if started {
        app.edit("Scrub scale", None);
    }
    for target in targets {
        let Some(node) = app.scene.get_mut(*target) else { continue };
        let mut s = Node::sane_scale(node.scale);
        let next = (crate::gizmo::get_axis(s, axis) + delta).max(Node::MIN_SCALE);
        set_component(&mut s, axis, next);
        node.scale = s;
    }
    app.touch();
    app.fields.clear();
}

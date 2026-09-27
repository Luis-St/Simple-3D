//! Dragging a field to change its number.

use super::*;
use simple3d_core::primitive::ParamKind;
use simple3d_core::unit::Unit;

/// The value of one horizontal pixel of scrub. Shift is fine, Ctrl coarse, as for the manipulator.
pub fn scrub_step(step: f64, fine: bool, coarse: bool) -> f64 {
    let factor = match (fine, coarse) {
        (true, _) => 0.1,
        (false, true) => 10.0,
        _ => 1.0,
    };
    step * factor / PIXELS_PER_STEP
}

/// A scrub gesture on a field. The whole drag is one undo step (snapshot at the start). The id is
/// held so running over another field never hands the gesture to it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scrub {
    pub id: Option<egui::Id>,
    /// The part of the drag a whole-number field could not take yet, carried so counts follow the
    /// pointer like other fields instead of rounding each frame away (or overshooting).
    pub carry: f64,
    /// Whether this gesture has moved the value yet, so a purely vertical press records no empty undo step.
    pub moved: bool,
}

/// One frame of a scrub: which field owns the gesture and how far it moved. The id is held since
/// any real edit runs off the field.
pub fn scrub_gesture(ui: &mut egui::Ui, response: &egui::Response, scrub: &mut Scrub, step: f64) -> Option<Scrubbed> {
    let id = response.id;
    if response.drag_started() {
        scrub.id = Some(id);
        // Start with nothing owed, so a leftover fraction is not spent on this field.
        scrub.carry = 0.0;
        scrub.moved = false;
    }
    if scrub.id != Some(id) {
        return None;
    }
    if !response.dragged() {
        scrub.id = None;
        scrub.carry = 0.0;
        scrub.moved = false;
        return None;
    }
    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    let (fine, coarse) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
    // Horizontal only: vertical movement is a hand steadying the drag, not a second axis.
    let delta = scrub_delta(response.drag_delta().x, step, fine, coarse);
    if delta == 0.0 && !scrub.moved {
        return None;
    }
    let started = !scrub.moved;
    scrub.moved = true;
    Some(Scrubbed { started, delta })
}

/// This frame's pointer movement in the field's own units.
pub fn scrub_delta(dx: f32, step: f64, fine: bool, coarse: bool) -> f64 {
    dx as f64 * scrub_step(step, fine, coarse)
}

/// The scrub increment for a field kind in its displayed unit: one millimetre, degree or segment.
pub fn scrub_increment(kind: ParamKind, unit: Unit) -> f64 {
    match kind {
        ParamKind::Length { .. } => unit.from_mm(1.0),
        ParamKind::Angle { .. } => 1.0,
        _ => 1.0,
    }
}

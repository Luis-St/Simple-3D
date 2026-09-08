//! Dragging a label to change the number beside it.

use super::*;
use simple3d_core::primitive::ParamKind;
use simple3d_core::unit::Unit;

/// How much one horizontal pixel of a label scrub is worth.
///
/// Shift is fine, Ctrl is coarse -- the same two modifiers the manipulator uses
/// for the same two meanings, so there is one thing to learn rather than two.
pub fn scrub_step(step: f64, fine: bool, coarse: bool) -> f64 {
    let factor = match (fine, coarse) {
        (true, _) => 0.1,
        (false, true) => 10.0,
        _ => 1.0,
    };
    step * factor / PIXELS_PER_STEP
}

/// A drag on a field's *label*, which scrubs the value.
///
/// The whole gesture is one undo step: the snapshot is taken when the drag
/// starts, and every frame after it only touches the scene. Forty snapshots for
/// one drag would make undo useless exactly where it is needed most. The id is
/// held so a pointer that runs off one label and over another cannot hand the
/// gesture to a field the user never grabbed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scrub {
    pub id: Option<egui::Id>,
    /// The part of the drag a whole-numbered field could not take yet.
    ///
    /// A count is stored as an integer and read back out of the model on every
    /// frame, so the fraction of a copy each frame of the drag is worth was
    /// rounded away sixty times a second rather than added up. A hand moving a
    /// pixel a frame rounded to nothing and the field never moved at all; a hand
    /// moving four rounded *up* on every frame and the field ran away from the
    /// pointer. Kept here instead, and spent on the frame it comes to a whole
    /// one, which is what makes a count follow the pointer at the same six
    /// pixels a step every other field does.
    pub carry: f64,
    /// Whether this gesture has moved the value yet.
    ///
    /// The frame that reports `started` is the frame a field records its one
    /// undo step on, so a gesture that has not moved anything must not report it:
    /// the field scrubs on the horizontal axis alone, and a press dragged
    /// straight down would otherwise leave a step behind that undoes nothing.
    pub moved: bool,
}

/// One frame of a scrub on a value box: which field owns the gesture, and how
/// far it has moved.
///
/// The id is held for the length of the drag so a pointer that runs off one
/// field and over another cannot hand the gesture to a field the user never
/// grabbed -- and any real edit does run off, six pixels to the millimetre.
pub fn scrub_gesture(ui: &mut egui::Ui, response: &egui::Response, scrub: &mut Scrub, step: f64) -> Option<Scrubbed> {
    let id = response.id;
    if response.drag_started() {
        scrub.id = Some(id);
        // Nothing is owed at the start of a gesture: a fraction left over from
        // the last one would be spent on this field's first frame.
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
    // The value follows the pointer sideways and nothing else. Vertical movement
    // is how a hand holds a horizontal drag steady, not a second axis to edit
    // on -- and a field that answered to both could not be dragged along a row
    // without wandering.
    let delta = scrub_delta(response.drag_delta().x, step, fine, coarse);
    if delta == 0.0 && !scrub.moved {
        return None;
    }
    let started = !scrub.moved;
    scrub.moved = true;
    Some(Scrubbed { started, delta })
}

/// This frame's pointer movement as a change in the field's own units.
pub fn scrub_delta(dx: f32, step: f64, fine: bool, coarse: bool) -> f64 {
    dx as f64 * scrub_step(step, fine, coarse)
}

/// The scrub increment for a field of a given kind, in the unit the field
/// shows. One millimetre in a millimetre document, one degree, one segment.
pub fn scrub_increment(kind: ParamKind, unit: Unit) -> f64 {
    match kind {
        ParamKind::Length { .. } => unit.from_mm(1.0),
        ParamKind::Angle { .. } => 1.0,
        _ => 1.0,
    }
}

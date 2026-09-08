//! The fields a row is made of, and the identity each one is given.

use crate::app::{App, Status};
use crate::ui::{self, Commit};
use simple3d_core::primitive::ParamKind;
use simple3d_core::unit::{format_angle, format_length, format_number};

/// The id the saved-kind box on a custom pattern answers to. Named for the same
/// reason the pattern tool's cross is: a test asks where it was drawn rather
/// than guessing at the id egui gave it.
pub(crate) fn saved_kind_id() -> egui::Id {
    egui::Id::new("pattern-saved-kind-box")
}

/// The id of a value field's scrub gesture, named after the value it scrubs
/// rather than taken from where the field sits in the layout. A gesture in
/// flight is remembered by this id (`App::scrub`), so a panel that relays itself
/// out mid-drag -- a section collapsing, a dock being resized -- must not change
/// it. It is also what lets a test put the pointer on a named field.
pub fn grip_id(name: &str) -> egui::Id {
    egui::Id::new(("scrub-grip", name))
}

/// One value field: a number that can be typed into or dragged, named by
/// `name` so the gesture survives the panel relaying itself out.
pub(crate) fn value_field(
    app: &mut App,
    ui: &mut egui::Ui,
    name: &str,
    field_id: egui::Id,
    shown: &str,
    step: f64,
) -> ui::Field {
    // The scrub state is lifted out and put back so the field can borrow the
    // buffers mutably without borrowing the whole application twice.
    let mut scrub = app.scrub;
    let outcome = app.fields.scrub_field(ui, field_id, grip_id(name), shown, step, &mut scrub);
    app.scrub = scrub;
    outcome
}

/// A number that is not a primitive's parameter -- the document's grid or step,
/// a segment count, a component of the 3D cursor or of the measure span.
///
/// These were egui's own `DragValue`, which is a different control wearing the
/// same theme: it drags on the vertical axis as well as the horizontal, it has
/// no coarse modifier, it reads no units and no deltas, and it silently swallows
/// what it cannot parse. One kind of number field in the panel means one set of
/// answers to all of that, so they come through the same field as every
/// dimension row.
///
/// `current` is in stored terms -- millimetres for a length, degrees for an
/// angle -- and so is the value handed to `apply`. `apply` is also told whether
/// this is the frame the gesture *began*, which is the frame that records the
/// undo step: recording on every frame would spend a whole drag's worth of
/// history on one edit.
pub(crate) struct Scalar<'a> {
    /// What the scrub gesture is remembered by, named after the value rather
    /// than taken from the layout -- see [`grip_id`].
    pub(crate) grip: &'a str,
    /// What the text field it opens into is remembered by.
    pub(crate) id: egui::Id,
    /// What the number is, which decides how it is written, what a typed entry
    /// may say, and where it is clamped.
    pub(crate) kind: ParamKind,
    /// What it holds now, in stored terms.
    pub(crate) current: f64,
    /// How much one step of the scrub is worth, in the unit the field shows.
    pub(crate) step: f64,
}

pub(crate) fn scalar_field(
    app: &mut App,
    ui: &mut egui::Ui,
    field: Scalar<'_>,
    mut apply: impl FnMut(&mut App, f64, bool),
) {
    let Scalar { grip, id: field_id, kind, current, step } = field;
    let unit = app.unit();
    let shown = match kind {
        ParamKind::Length { .. } => format_length(current, unit),
        ParamKind::Angle { .. } => format_angle(current),
        _ => format_number(current, 0),
    };
    let outcome = value_field(app, ui, grip, field_id, &shown, step);
    if let Some(scrubbed) = outcome.scrubbed {
        // A whole number is read back out of the model on every frame, so the
        // fraction of a step each frame is worth has to be carried rather than
        // rounded away -- see `scrub_param`, which carries it the same way.
        let whole = matches!(kind, ParamKind::Count { .. });
        let carried = if whole { app.scrub.carry } else { 0.0 };
        let displayed = match kind {
            ParamKind::Length { .. } => unit.from_mm(current),
            _ => current,
        };
        let wanted = displayed + scrubbed.delta + carried;
        let next = ui::param_number(ui::value_from_display(kind, unit, wanted));
        if whole {
            app.scrub.carry = (wanted - next).clamp(-1.0, 1.0);
        }
        apply(app, next, scrubbed.started);
    }
    if let Some(text) = outcome.committed {
        match ui::commit_param(&text, kind, unit, current) {
            Commit::Value(value) => {
                app.fields.accept(field_id);
                apply(app, ui::param_number(value), true);
            }
            Commit::Revert => {
                app.fields.reject(field_id, text.clone());
                app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
            }
        }
    }
}

/// What a scrubbed scene setting does with the frame it is on: the first frame
/// of the gesture takes the one undo snapshot the whole drag gets, and every
/// frame after it only marks the scene for re-evaluation.
pub(crate) fn edit_or_touch(app: &mut App, started: bool, label: &str, key: &str) {
    if started {
        app.edit(label, Some(key));
    } else {
        app.touch();
    }
}

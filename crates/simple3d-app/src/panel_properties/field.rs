//! The fields a row is made of, and the identity each one is given.

use crate::app::{App, Status};
use crate::ui::{self, Commit};
use simple3d_core::primitive::ParamKind;
use simple3d_core::unit::{format_angle, format_length, format_number};

/// The saved-kind box's id on a custom pattern, for tests.
pub(crate) fn saved_kind_id() -> egui::Id {
    egui::Id::new("pattern-saved-kind-box")
}

/// A value field's scrub id, named after the value rather than its layout position, so a
/// mid-drag relayout keeps the gesture (`App::scrub`); also lets tests find named fields.
pub fn grip_id(name: &str) -> egui::Id {
    egui::Id::new(("scrub-grip", name))
}

/// One value field, typed or dragged, named by `name` so the gesture survives relayout.
pub(crate) fn value_field(
    app: &mut App,
    ui: &mut egui::Ui,
    name: &str,
    field_id: egui::Id,
    shown: &str,
    step: f64,
) -> ui::Field {
    // The scrub state is lifted out so the field can borrow the buffers without borrowing the app twice.
    let mut scrub = app.scrub;
    let outcome = app.fields.scrub_field(ui, field_id, grip_id(name), shown, step, &mut scrub);
    app.scrub = scrub;
    outcome
}

/// A number that is not a primitive parameter (grid, step, segments, cursor, measure span), using
/// the same field as dimensions instead of egui's `DragValue`. `current` and the value handed to
/// `apply` are in stored terms; `apply` is told whether the gesture began this frame, the only one
/// that records undo.
pub(crate) struct Scalar<'a> {
    /// The scrub gesture's name, after the value (see [`grip_id`]).
    pub(crate) grip: &'a str,
    /// The id of the text field it opens into.
    pub(crate) id: egui::Id,
    /// The number's kind: how it is written, parsed and clamped.
    pub(crate) kind: ParamKind,
    /// Its current value, in stored terms.
    pub(crate) current: f64,
    /// One scrub step, in the field's displayed unit.
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
        // Whole numbers carry the fraction between frames, as `scrub_param` does.
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

/// A scrubbed setting's frame: the first records the drag's one undo snapshot, the rest only mark
/// the scene dirty.
pub(crate) fn edit_or_touch(app: &mut App, started: bool, label: &str, key: &str) {
    if started {
        app.edit(label, Some(key));
    } else {
        app.touch();
    }
}

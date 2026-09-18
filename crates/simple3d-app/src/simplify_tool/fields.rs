//! One field of the plan, and what it is called.

use super::*;
use crate::app::{App, Status};
use crate::ui;

/// A row's name, carrying what the number beside it means.
///
/// The hover is on the label rather than on the field, exactly as the
/// properties panel puts it: the field is dragged and typed into, and a tooltip
/// that appears under the pointer halfway through a drag is a tooltip in the
/// way of the thing it is describing.
pub(crate) fn label(ui: &mut egui::Ui, name: &str, hover: &str) {
    ui.label(name).on_hover_text(hover);
}

/// A number that is dragged to change it and clicked to type into it -- the
/// same control the properties panel's rows are, through the same buffers, so
/// the two answer the pointer identically.
///
/// What it edits is not a document parameter, so there is no undo step to take:
/// the value goes into the plan the window holds, and the next run of the
/// simplification picks it up.
pub(crate) fn number(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64) {
    let unit = app.unit();
    let shown = match kind {
        ParamKind::Length { .. } => simple3d_core::unit::format_length(*value, unit),
        ParamKind::Angle { .. } => simple3d_core::unit::format_angle(*value),
        // A percentage and a count are whole numbers, and a percentage written
        // "50.0" invites a decimal the field would only round away.
        _ => format!("{}", value.round() as i64),
    };
    let id = egui::Id::new(("simplify-field", name));
    let step = ui::scrub_increment(kind, unit);
    // The scrub state is lifted out and put back so the field can borrow the
    // buffers mutably without borrowing the whole application twice.
    let mut scrub = app.scrub;
    let outcome = ui
        .scope(|ui| {
            ui.set_max_width(FIELD_WIDTH);
            app.fields.scrub_field(ui, id, crate::panel_properties::grip_id(name), &shown, step, &mut scrub)
        })
        .inner;
    app.scrub = scrub;
    if let Some(scrubbed) = outcome.scrubbed {
        let displayed = match kind {
            ParamKind::Length { .. } => unit.from_mm(*value),
            _ => *value,
        };
        *value = ui::param_number(ui::value_from_display(kind, unit, displayed + scrubbed.delta));
    }
    if let Some(text) = outcome.committed {
        match ui::commit_param(&text, kind, unit, *value) {
            ui::Commit::Value(committed) => {
                app.fields.accept(id);
                *value = ui::param_number(committed);
            }
            ui::Commit::Revert => {
                app.fields.reject(id, text.clone());
                app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
            }
        }
    }
}

/// A field that only means anything while its checkbox is ticked, drawn beside
/// it and greyed out until it is.
///
/// Greyed rather than gone: a row that appears and disappears moves everything
/// under it, and the number is worth reading -- and worth having kept -- while
/// the limit it belongs to is switched off.
pub(crate) fn optional(app: &mut App, ui: &mut egui::Ui, on: &mut bool, name: &str, kind: ParamKind, value: &mut f64) {
    ui.checkbox(on, "");
    let enabled = *on;
    ui.add_enabled_ui(enabled, |ui| number(app, ui, name, kind, value));
}

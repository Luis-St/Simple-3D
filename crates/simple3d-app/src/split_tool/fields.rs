//! One field of the cut, and what it is called.

use super::*;
use crate::app::{App, Status};
use crate::ui;
use simple3d_core::primitive::ParamKind;

/// A row's name, carrying what the number beside it means.
///
/// The hover is on the label rather than on the field, exactly as the
/// properties panel puts it: the field is dragged and typed into, and a tooltip
/// that appears under the pointer halfway through a drag is a tooltip in the
/// way of the thing it is describing.
pub(crate) fn label(ui: &mut egui::Ui, name: &str, hover: &str) {
    ui.label(name).on_hover_text(hover);
}

/// What one field is called. The name is what the drag gesture is remembered
/// by, so it carries the cut it belongs to: two cuts have two Size fields, and
/// a gesture handed from one to the other would edit the wrong one.
pub(crate) fn field_name(index: usize, part: &str) -> String {
    format!("split-{index}-{part}")
}

/// A number that is dragged to change it and clicked to type into it -- the
/// same control the properties panel's rows are, through the same buffers, so
/// the two answer the pointer identically (issue 82).
///
/// The tiling it edits is not a document parameter, so there is no undo step to
/// take and nothing to mark for re-evaluation: the value goes straight into the
/// plan the window holds, and the model is not touched until Split is pressed.
pub(crate) fn number(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64) {
    let unit = app.unit();
    let shown = match kind {
        ParamKind::Length { .. } => simple3d_core::unit::format_length(*value, unit),
        _ => simple3d_core::unit::format_angle(*value),
    };
    let id = egui::Id::new(("split-field", name));
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

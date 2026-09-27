//! The simplify tool's fields and their names.

use super::*;
use crate::app::App;
use crate::popup;

/// Scopes fields to this tool, so a drag passed between tool windows cannot edit the wrong number.
const SCOPE: &str = "simplify-field";

pub(crate) use popup::label;

pub(crate) fn number(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64) {
    popup::number(app, ui, (SCOPE, name), FIELD_WIDTH, kind, value);
}

pub(crate) fn optional(app: &mut App, ui: &mut egui::Ui, on: &mut bool, name: &str, kind: ParamKind, value: &mut f64) {
    popup::optional(app, ui, on, (SCOPE, name), FIELD_WIDTH, kind, value);
}

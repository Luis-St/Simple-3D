//! One field of the plan, and what it is called: the popup's own controls, on
//! this tool's numbers.

use super::*;
use crate::app::App;
use crate::popup;

/// Which fields are this tool's, so a drag handed between two tools' windows
/// cannot edit the wrong number.
const SCOPE: &str = "simplify-field";

pub(crate) use popup::label;

pub(crate) fn number(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64) {
    popup::number(app, ui, (SCOPE, name), FIELD_WIDTH, kind, value);
}

pub(crate) fn optional(app: &mut App, ui: &mut egui::Ui, on: &mut bool, name: &str, kind: ParamKind, value: &mut f64) {
    popup::optional(app, ui, on, (SCOPE, name), FIELD_WIDTH, kind, value);
}

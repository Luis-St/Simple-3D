//! The split tool's fields and their names.

use super::*;
use crate::app::App;
use crate::popup;
use simple3d_core::primitive::ParamKind;

/// Scopes fields to this tool, so a drag passed between tool windows cannot edit the wrong number.
const SCOPE: &str = "split-field";

pub(crate) use popup::label;

/// A field's name, which the drag is remembered by, so it includes the cut: two cuts have two Size fields.
pub(crate) fn field_name(index: usize, part: &str) -> String {
    format!("split-{index}-{part}")
}

pub(crate) fn number(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64) {
    popup::number(app, ui, (SCOPE, name), FIELD_WIDTH, kind, value);
}

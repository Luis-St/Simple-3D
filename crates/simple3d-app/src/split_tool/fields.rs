//! One field of the cut, and what it is called: the popup's own controls, on
//! this tool's numbers.

use super::*;
use crate::app::App;
use crate::popup;
use simple3d_core::primitive::ParamKind;

/// Which fields are this tool's, so a drag handed between two tools' windows
/// cannot edit the wrong number.
const SCOPE: &str = "split-field";

pub(crate) use popup::label;

/// What one field is called. The name is what the drag gesture is remembered
/// by, so it carries the cut it belongs to: two cuts have two Size fields, and
/// a gesture handed from one to the other would edit the wrong one.
pub(crate) fn field_name(index: usize, part: &str) -> String {
    format!("split-{index}-{part}")
}

pub(crate) fn number(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64) {
    popup::number(app, ui, (SCOPE, name), FIELD_WIDTH, kind, value);
}

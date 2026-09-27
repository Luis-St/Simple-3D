//! One panel row: its label, where its fields end, and whether they stack below the label.

use super::*;
use crate::theme::{self, token};

/// How a section renders shared parameter rows: its undo label and its gesture scope.
#[derive(Clone, Copy)]
pub(crate) struct RowStyle {
    /// The undo step's name; a pattern's edits are not measurements.
    pub(super) edit_label: &'static str,
    /// Distinguishes this row's scrub gesture from the same parameter's row elsewhere, since the
    /// creation tool and the properties panel show the same numbers at once.
    pub(super) grip_scope: &'static str,
}

impl RowStyle {
    /// The gesture scope, for controls drawn beside these rows.
    pub(crate) fn scope(self) -> &'static str {
        self.grip_scope
    }
}

/// A shape's dimensions.
pub(crate) const DIMENSION_ROW: RowStyle = RowStyle { edit_label: "Set measurement", grip_scope: "" };

/// A pattern's numbers.
pub(crate) const PATTERN_ROW: RowStyle = RowStyle { edit_label: "Set pattern", grip_scope: "" };

/// The same rows, in the creation tool's window (issue 67).
pub(crate) const PATTERN_TOOL_ROW: RowStyle = RowStyle { grip_scope: "tool", ..PATTERN_ROW };

/// The room left on the current line, up to the row's pinned right edge. Not
/// [`egui::Ui::available_width`], which in a wrapped layout reports a new line's width (issue 57).
pub(crate) fn room_left(ui: &egui::Ui) -> f32 {
    (ui.max_rect().right() - ui.cursor().left()).max(0.0)
}

/// A row name with its unit in brackets, "Width (mm)", so fields keep the same width whatever
/// the unit.
pub(crate) fn named(label: &str, unit: &str) -> String {
    if unit.is_empty() {
        return label.to_string();
    }
    // A name already ending in brackets gets the unit inside them rather than a second pair.
    match label.strip_suffix(')') {
        Some(head) => format!("{head}, {unit})"),
        None => format!("{label} ({unit})"),
    }
}

/// A width a control would like, capped at what the row has left.
pub(crate) fn fits(ui: &egui::Ui, wanted: f32) -> f32 {
    wanted.min(room_left(ui)).max(48.0)
}

/// Whether the panel is too narrow for a label column beside the fields.
pub(crate) fn stacked(ui: &egui::Ui) -> bool {
    ui.available_width() < STACK_BELOW
}

/// A row's name in its own column, wrapped within it.
pub(crate) fn row_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let galley = ui.painter().layout(
        text.to_string(),
        egui::FontId::proportional(theme::font::LABEL),
        token::TEXT_LO,
        LABEL_WIDTH,
    );
    // The column keeps its width and grows downward for a two-line name.
    let height = galley.size().y.max(theme::metric::INPUT_ROW);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(LABEL_WIDTH, height), egui::Sense::hover());
    ui.painter().galley(egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0), galley, token::TEXT_LO);
    response
}

/// The right edge a row is pinned to before layout. A wrapped ui grows its `max_rect` to hold
/// overflow, so an unpinned row kept overflowing. Public so controls beside the rows end where
/// they do.
pub(crate) fn row_right_edge(ui: &egui::Ui) -> f32 {
    ui.max_rect().right().min(ui.clip_rect().right() - EDGE_PAD)
}

/// Like [`field_row`], but with the controls in their own column beside the name, wrapping
/// within it, for long rows of choices.
pub(crate) fn field_row_boxed(ui: &mut egui::Ui, label: &str, hover: &str, contents: impl FnOnce(&mut egui::Ui)) {
    // Too narrow for a name column: the stacked layout is already this shape.
    if stacked(ui) {
        field_row(ui, label, hover, contents);
        return;
    }
    let right = row_right_edge(ui);
    // Top-aligned, so the name sits beside the first line of options.
    ui.horizontal_top(|ui| {
        ui.set_max_width((right - ui.max_rect().left()).max(LABEL_WIDTH));
        let name = row_label(ui, label);
        if !hover.is_empty() {
            name.on_hover_text(hover);
        }
        ui.vertical(|ui| ui.horizontal_wrapped(contents));
    });
}

/// One property row: a fixed name column with the controls beside it, or the name on its own
/// line when too narrow (issue 51). Controls wrap rather than overflow.
pub(crate) fn field_row(ui: &mut egui::Ui, label: &str, hover: &str, contents: impl FnOnce(&mut egui::Ui)) {
    if stacked(ui) {
        ui.vertical(|ui| {
            if !label.is_empty() {
                let name = ui.add(
                    egui::Label::new(egui::RichText::new(label).size(theme::font::LABEL).color(token::TEXT_LO))
                        .selectable(false)
                        .wrap(),
                );
                if !hover.is_empty() {
                    name.on_hover_text(hover);
                }
            }
            ui.horizontal_wrapped(contents);
        });
        return;
    }
    let right = row_right_edge(ui);
    ui.horizontal_wrapped(|ui| {
        ui.set_max_width((right - ui.max_rect().left()).max(LABEL_WIDTH));
        let name = row_label(ui, label);
        if !hover.is_empty() {
            name.on_hover_text(hover);
        }
        contents(ui);
    });
}

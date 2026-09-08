//! One row of the panel: its label, where its fields end, and whether they
//! fit beside the label or have to stack below it.

use super::*;
use crate::theme::{self, token};

/// How a section renders the parameter rows it shares with every other
/// section: what its edits are called in the undo history, and what tells its
/// gestures apart from the same rows drawn somewhere else.
#[derive(Clone, Copy)]
pub(crate) struct RowStyle {
    /// What the undo step is called. A primitive's choices really are
    /// measurements -- "outer diameter or wall thickness" -- but a pattern's
    /// are its kind and its axis, and filing those under "Set measurement" made
    /// the undo history describe something the user had not done.
    pub(super) edit_label: &'static str,
    /// What tells this row's scrub gesture apart from the same parameter's row
    /// somewhere else. A gesture is remembered by the value it drags rather
    /// than by where the field sits, which is what lets a panel relay itself out
    /// mid-drag -- but the creation tool shows a pattern's stage numbers while
    /// the properties panel behind it is showing the very same ones, and two
    /// widgets cannot answer to one name in one frame.
    pub(super) grip_scope: &'static str,
}

/// A shape's dimensions.
pub(crate) const DIMENSION_ROW: RowStyle = RowStyle { edit_label: "Set measurement", grip_scope: "" };

/// A pattern's numbers.
pub(crate) const PATTERN_ROW: RowStyle = RowStyle { edit_label: "Set pattern", grip_scope: "" };

/// The same rows, in the creation tool's own window (issue 67).
pub(crate) const PATTERN_TOOL_ROW: RowStyle = RowStyle { grip_scope: "tool", ..PATTERN_ROW };

/// How much room is left on the line a row is currently laying out on:
/// from where the next control will start to the row's own right-hand edge.
///
/// Not [`egui::Ui::available_width`]. In a *wrapped* horizontal layout that
/// reports the width a new line would have, not what is left on this one -- so
/// a field sized by it started after the label column and still asked for the
/// whole row, and ran that far past the panel's edge (issue 57). The row's
/// right-hand edge is pinned by [`field_row`] before anything is drawn in it,
/// which is what makes this exact.
pub(crate) fn room_left(ui: &egui::Ui) -> f32 {
    (ui.max_rect().right() - ui.cursor().left()).max(0.0)
}

/// A row's name with the unit its value is written in, in brackets on the end:
/// "Width (mm)", "Rotation (deg)".
///
/// Every value row in the panel names its unit this way rather than writing it
/// after the field. A suffix beside the field takes its width out of the field,
/// and it takes a different width for every unit and for none at all -- so a
/// column that mixed a length, an angle and a plain count had a different field
/// width on every line of it.
pub(crate) fn named(label: &str, unit: &str) -> String {
    if unit.is_empty() {
        return label.to_string();
    }
    // Half the names in the registry already end in brackets -- "Width (X)",
    // "Top diameter (0 = point)" -- and a second pair straight after the first
    // reads as a mistake, so the unit joins the ones that are there.
    match label.strip_suffix(')') {
        Some(head) => format!("{head}, {unit})"),
        None => format!("{label} ({unit})"),
    }
}

/// A width a control would like, capped at what the row actually has left.
pub(crate) fn fits(ui: &egui::Ui, wanted: f32) -> f32 {
    wanted.min(room_left(ui)).max(48.0)
}

/// Whether the panel is too narrow for a label column beside the fields.
pub(crate) fn stacked(ui: &egui::Ui) -> bool {
    ui.available_width() < STACK_BELOW
}

/// A row's name, in its own column, wrapped inside that column rather than
/// running under the field beside it.
pub(crate) fn row_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let galley = ui.painter().layout(
        text.to_string(),
        egui::FontId::proportional(theme::font::LABEL),
        token::TEXT_LO,
        LABEL_WIDTH,
    );
    // The column keeps its width whatever the name does with it, and grows
    // downwards for a name that needed two lines, so the field beside it is
    // still where a field is expected.
    let height = galley.size().y.max(theme::metric::INPUT_ROW);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(LABEL_WIDTH, height), egui::Sense::hover());
    ui.painter().galley(egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0), galley, token::TEXT_LO);
    response
}

/// One property row: what it is called, and whatever edits it.
///
/// Given the width for it, the name is a fixed column with the controls beside
/// it, so a column of numbers lines up down the panel. Dragged in narrower than
/// that, the name goes on its own line and the controls take the full width
/// underneath, rather than the two of them squeezing a field down to nothing or
/// pushing it off the panel edge (issue 51). Either way the controls are laid
/// out wrapped, so a row of choices that no longer fits across breaks onto a
/// second line instead of overflowing.
/// The right-hand edge a row is pinned to, fixed before anything is laid out
/// inside it. A wrapped horizontal ui lets its `max_rect` grow to hold whatever
/// overflowed it, so a row that ran off the panel once went on doing so for as
/// long as the panel was open. Pinned here it cannot, and `room_left` is exact.
///
/// Public because anything drawn *beside* these rows has to end where they do:
/// a control on its own line whose right edge is a few pixels out from the
/// fields above it reads as a mistake, and one constant answering for both is
/// the only way they cannot drift apart.
pub(crate) fn row_right_edge(ui: &egui::Ui) -> f32 {
    ui.max_rect().right().min(ui.clip_rect().right() - EDGE_PAD)
}

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

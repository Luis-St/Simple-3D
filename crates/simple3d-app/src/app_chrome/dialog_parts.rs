//! The pieces every dialog is built from.

use crate::theme;

/// A label that claims a whole column of a dialog's grid, so the rows below it
/// line up with the rows above and the fields beside them start in the same
/// place. A plain `ui.label` takes the width of its own text, which is what left
/// each grid measuring its own indent.
pub(crate) fn label_cell(ui: &mut egui::Ui, text: &str, width: f32) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, theme::metric::INPUT_ROW),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            // The point of the cell is the width: without this the region
            // shrinks back to the text and the column is ragged again.
            ui.set_min_width(width);
            ui.add(egui::Label::new(text).truncate().selectable(false));
        },
    );
}

/// What a dialog's window is: what it is called, how big it opens and whether it
/// can be resized. One argument rather than four, so the call sites read as the
/// window they describe.
pub(crate) struct DialogSpec<'a> {
    pub(super) key: &'a str,
    pub(super) title: &'a str,
    /// The size the window opens at. With `fit_height` the height is only a
    /// starting point: the contents settle it on the first frame.
    pub(super) size: egui::Vec2,
    pub(super) resizable: bool,
    /// Take the height from the contents rather than from `size`.
    pub(super) fit_height: bool,
    /// The smallest the window may be dragged to, for a resizable one whose
    /// contents stop making sense below a size. `None` leaves it to the window
    /// manager, which is right for a dialog that is a sentence and two buttons.
    pub(super) min_size: Option<egui::Vec2>,
}

/// The buttons of a dialog, laid out the one way they are laid out everywhere:
/// along the foot of the window, right-aligned, the affirmative one last.
///
/// The contents are added *right to left*, so the closure names the rightmost
/// button first -- the one Enter would press if a dialog had a default -- and
/// the one that walks away from the dialog ends up furthest left, where the eye
/// arrives last.
pub(crate) fn action_row(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = theme::metric::GAP * 2.0;
        contents(ui);
    });
}

/// Cancel, at the far end of a dialog's button row from the buttons that go
/// through with it. The row is laid out right to left, so what is left of it
/// after the other buttons is claimed here and filled left to right: the
/// button that abandons the dialog is not next to the one that commits it, and
/// cannot be hit by aiming for it.
pub(crate) fn cancel_at_left(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), contents);
}

/// The status bar's separator: a dot, not a rule. A vertical line every few
/// words turns a single sentence of state into a row of boxes.
pub(crate) fn dot(ui: &mut egui::Ui) {
    ui.add(
        egui::Label::new(egui::RichText::new("\u{00B7}").size(theme::font::LABEL).color(theme::token::SURFACE_3))
            .selectable(false),
    );
}

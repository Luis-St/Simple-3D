//! The pieces every dialog is built from.

use crate::theme;

/// A label claiming a whole column width, so a dialog grid's fields line up; `ui.label` would
/// only take its text's width.
pub(crate) fn label_cell(ui: &mut egui::Ui, text: &str, width: f32) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, theme::metric::INPUT_ROW),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            // The width is the point; without it the column goes ragged.
            ui.set_min_width(width);
            ui.add(egui::Label::new(text).truncate().selectable(false));
        },
    );
}

/// A dialog window's title, size and resizability, as one argument.
pub(crate) struct DialogSpec<'a> {
    pub(super) key: &'a str,
    pub(super) title: &'a str,
    /// The opening size; with `fit_height` the height only starts here.
    pub(super) size: egui::Vec2,
    pub(super) resizable: bool,
    /// Take the height from the contents rather than from `size`.
    pub(super) fit_height: bool,
    /// The smallest a resizable window may be dragged to; `None` leaves it to the window manager.
    pub(super) min_size: Option<egui::Vec2>,
}

/// A dialog's buttons along the foot, right-aligned, laid out right to left so the closure names
/// the affirmative one first.
pub(crate) fn action_row(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = theme::metric::GAP * 2.0;
        contents(ui);
    });
}

/// Cancel at the far left of a dialog's button row, away from the committing buttons.
pub(crate) fn cancel_at_left(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), contents);
}

/// The status bar's separator: a dot, since rules would box a sentence up.
pub(crate) fn dot(ui: &mut egui::Ui) {
    ui.add(
        egui::Label::new(egui::RichText::new("\u{00B7}").size(theme::font::LABEL).color(theme::token::SURFACE_3))
            .selectable(false),
    );
}

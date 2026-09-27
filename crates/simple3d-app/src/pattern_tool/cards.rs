//! Shared card pieces: a heading with a cross, an add chip, and a note line.

use super::*;
use crate::panel_properties::row_right_edge;
use crate::theme::{self};

/// A card heading: name, short summary and a removing cross. Returns whether the cross was pressed.
pub(crate) fn card_heading(ui: &mut egui::Ui, name: &str, summary: &str, cross_id: egui::Id, hover: &str) -> bool {
    let mut clicked = false;
    capped_row(ui, |ui| {
        ui.add(egui::Label::new(theme::header_text(name)).selectable(false));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let cross = ui.add_sized(egui::Vec2::splat(CARD_CROSS), egui::Button::new("\u{00d7}")).on_hover_text(hover);
            // Hover-only; the button answers the pointer.
            ui.interact(cross.rect, cross_id, egui::Sense::hover());
            clicked = cross.clicked();
        });
    });
    if !summary.is_empty() {
        note(ui, summary);
    }
    clicked
}

/// An add chip, identified by `id` so tests can locate it. Returns whether it was pressed.
pub(crate) fn add_chip(ui: &mut egui::Ui, text: &str, hover: &str, id: egui::Id) -> bool {
    let chip = theme::choice(ui, false, text).on_hover_text(hover);
    // Hover-only; the chip answers the pointer.
    ui.interact(chip.rect, id, egui::Sense::hover());
    chip.clicked()
}

/// A quiet note under a card's numbers, wrapped to the column.
pub(crate) fn note(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(theme::hint(text)).selectable(false).wrap());
}

/// A row ending where the field rows below do (see [`heading`]).
pub(super) fn capped_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let right = row_right_edge(ui);
    ui.horizontal(|ui| {
        ui.set_max_width((right - ui.max_rect().left()).max(0.0));
        add(ui);
    });
}

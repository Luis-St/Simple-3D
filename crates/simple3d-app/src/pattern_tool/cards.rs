//! The pieces every card in the builders is made of: a heading with a cross,
//! a chip that adds, and a quiet line of English.

use super::*;
use crate::panel_properties::row_right_edge;
use crate::theme::{self};

/// A card's heading inside the builder: its name, what it currently does in a
/// few words, and the cross that takes it off. Returns whether the cross was
/// pressed.
pub(crate) fn card_heading(ui: &mut egui::Ui, name: &str, summary: &str, cross_id: egui::Id, hover: &str) -> bool {
    let mut clicked = false;
    capped_row(ui, |ui| {
        ui.add(egui::Label::new(theme::header_text(name)).selectable(false));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let cross = ui.add_sized(egui::Vec2::splat(CARD_CROSS), egui::Button::new("\u{00d7}")).on_hover_text(hover);
            // It senses nothing; the button answers the pointer.
            ui.interact(cross.rect, cross_id, egui::Sense::hover());
            clicked = cross.clicked();
        });
    });
    if !summary.is_empty() {
        note(ui, summary);
    }
    clicked
}

/// A chip that adds something to the builder, named by `id` rather than found
/// by the word on it, so a test can ask where it was drawn. Returns whether it
/// was pressed.
pub(crate) fn add_chip(ui: &mut egui::Ui, text: &str, hover: &str, id: egui::Id) -> bool {
    let chip = theme::choice(ui, false, text).on_hover_text(hover);
    // It senses nothing; the chip answers the pointer.
    ui.interact(chip.rect, id, egui::Sense::hover());
    chip.clicked()
}

/// A quiet line of English under a card's numbers, wrapped to the column.
pub(crate) fn note(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(theme::hint(text)).selectable(false).wrap());
}

/// A row that ends where the field rows under it do (see [`heading`]).
pub(super) fn capped_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let right = row_right_edge(ui);
    ui.horizontal(|ui| {
        ui.set_max_width((right - ui.max_rect().left()).max(0.0));
        add(ui);
    });
}

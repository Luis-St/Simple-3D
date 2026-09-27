//! A titled section of the panel.

use crate::theme::{self, token};

/// A collapsible right-dock panel: header bar and a padded body drawn only when open.
pub(crate) fn section(ui: &mut egui::Ui, name: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    section_titled(ui, name, "", add_contents)
}

/// A section with a quieter note on the right, such as the primitive type beside "Dimensions".
pub(crate) fn section_titled(ui: &mut egui::Ui, name: &str, note: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    let id = ui.id().with(("section", name));
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    let header = theme::panel_header(ui, name, |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        theme::twisty(ui.painter(), rect.center(), open, token::TEXT_LO);
        if !note.is_empty() {
            ui.add(egui::Label::new(theme::hint(note)).selectable(false));
        }
    });
    if header.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    if !open {
        return;
    }
    egui::Frame::NONE.inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 8 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
        add_contents(ui);
    });
}

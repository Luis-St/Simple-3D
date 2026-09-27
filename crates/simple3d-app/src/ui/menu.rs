//! Menu entries and dialog buttons.

use simple3d_core::keymap::{Command, Keymap};

/// A menu entry's label: the command's name and its current binding (spec section 8.2), separated
/// by a tab that `menu_entry` splits on.
pub fn menu_label(keymap: &Keymap, command: Command) -> String {
    match keymap.binding(command) {
        Some(chord) => format!("{}\t{}", command.label(), chord),
        None => command.label().to_string(),
    }
}

/// Split a menu label into the action and its key binding.
pub fn split_menu_label(label: &str) -> (&str, &str) {
    match label.split_once('\t') {
        Some((action, shortcut)) => (action, shortcut.trim()),
        None => (label, ""),
    }
}

/// One menu entry: the action on the left and its binding right-aligned in a keycap outline, so
/// bindings line up and read as keys.
pub fn menu_entry(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let (action, shortcut) = split_menu_label(label);
    let font = egui::FontId::monospace(crate::theme::font::SMALL);
    let mut button = egui::Button::new(action);
    if !shortcut.is_empty() {
        button = button.shortcut_text(egui::RichText::new(shortcut).font(font.clone()));
    }
    let response = ui.add_enabled(enabled, button);
    if !shortcut.is_empty() {
        // Painted after the button, so it can only be a stroke; a fill would cover the text.
        let width = ui.fonts(|fonts| fonts.layout_no_wrap(shortcut.to_string(), font, egui::Color32::WHITE).size().x);
        let right = response.rect.right() - ui.spacing().button_padding.x;
        let box_rect = egui::Rect::from_center_size(
            egui::pos2(right - width / 2.0, response.rect.center().y),
            egui::vec2(width, crate::theme::font::SMALL + 2.0),
        )
        .expand2(egui::vec2(4.0, 2.0));
        let colour = if enabled { crate::theme::token::SURFACE_3 } else { crate::theme::token::SURFACE_2 };
        ui.painter().rect_stroke(box_rect, 3.0, egui::Stroke::new(1.0_f32, colour), egui::StrokeKind::Inside);
    }
    response
}

/// A dialog action-row button, all the same size so the row is even (issue 63).
pub fn dialog_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let size = egui::vec2(crate::theme::metric::DIALOG_BUTTON_WIDTH, crate::theme::metric::DIALOG_BUTTON);
    ui.add_enabled(enabled, egui::Button::new(label).min_size(size))
}

/// The toolkit key a name stands for (inverse of `Key::name`), for checking held keys in the
/// snap-while-held mode (issue 68).
pub fn key_from_name(name: &str) -> Option<egui::Key> {
    egui::Key::ALL.iter().copied().find(|k| k.name() == name)
}

//! Menu entries and dialog buttons.

use simple3d_core::keymap::{Command, Keymap};

/// The label to put on a menu entry: the command's name plus its *current*
/// binding, never a hardcoded one (spec section 8.2). The two halves are kept
/// apart by a tab, and `menu_entry` is what draws them.
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

/// One menu entry: the action on the left, its key binding boxed at the right
/// edge of the menu.
///
/// A tab between the two put them one word apart, wherever that happened to
/// land, so a column of entries had its bindings scattered down the middle and
/// nothing said which text was a key and which was the name of the command.
/// The binding is now pushed to the right by a growing spacer -- so every
/// entry's binding lines up with every other's -- and drawn inside a keycap
/// outline, so it reads as a key rather than as more of the sentence.
pub fn menu_entry(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let (action, shortcut) = split_menu_label(label);
    let font = egui::FontId::monospace(crate::theme::font::SMALL);
    let mut button = egui::Button::new(action);
    if !shortcut.is_empty() {
        button = button.shortcut_text(egui::RichText::new(shortcut).font(font.clone()));
    }
    let response = ui.add_enabled(enabled, button);
    if !shortcut.is_empty() {
        // The outline is painted after the button, so it can only ever be a
        // stroke: a filled box here would cover the text already drawn under it.
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

/// Translate an egui key press into the chord form the keymap stores. Uses
/// egui's own key names, so there is no translation table to drift out of date.
/// A button in a dialog's action row: every one the same size, so a row of them
/// is a row and not a ragged line (issue 63).
pub fn dialog_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let size = egui::vec2(crate::theme::metric::DIALOG_BUTTON_WIDTH, crate::theme::metric::DIALOG_BUTTON);
    ui.add_enabled(enabled, egui::Button::new(label).min_size(size))
}

/// The toolkit key a name stands for, the inverse of `Key::name`. Used to ask
/// whether a bound key is being held right now -- for the snap-while-held mode
/// (issue 68), where a binding is a key to hold rather than a press to react to.
pub fn key_from_name(name: &str) -> Option<egui::Key> {
    egui::Key::ALL.iter().copied().find(|k| k.name() == name)
}

//! Accepting or rejecting typed text, and what a click selects.

use super::*;

impl FieldBuffers {
    /// The field took the value: drop any mark.
    pub fn accept(&mut self, id: egui::Id) {
        self.errors.remove(&id);
    }

    /// The field's text was unreadable: keep and mark it, never clear it or touch the model.
    pub fn reject(&mut self, id: egui::Id, text: String) {
        self.buffers.insert(id, text);
        self.errors.insert(id);
    }

    /// Whether a field is marked as rejected.
    pub fn is_rejected(&self, id: egui::Id) -> bool {
        self.errors.contains(&id)
    }

    /// Forget one field's draft after something else wrote its value (a Reset), or the draft would be
    /// committed over it on leaving.
    pub fn forget(&mut self, id: egui::Id) {
        self.buffers.remove(&id);
        self.errors.remove(&id);
        self.editing.remove(&id);
        self.opening.remove(&id);
    }

    /// Forget every draft, when the selection changes.
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.errors.clear();
        self.editing.clear();
        self.opening.clear();
    }
}

/// Select a just-opened value field's whole text, so typing replaces it (typing 12 into 20 gave
/// 2012). If the text field has not been drawn yet, the caret stays at the end.
pub(crate) fn select_whole_value(ui: &egui::Ui, id: egui::Id, text: &str) {
    let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) else { return };
    let whole =
        egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
    state.cursor.set_char_range(Some(whole));
    egui::TextEdit::store_state(ui.ctx(), id, state);
}

//! Accepting or rejecting what was typed, and what a click selects.

use super::*;

impl FieldBuffers {
    /// The field took the value: drop any mark it was wearing.
    pub fn accept(&mut self, id: egui::Id) {
        self.errors.remove(&id);
    }

    /// The field's text could not be read. Put it back and mark it -- never
    /// clear it, and never touch the model.
    pub fn reject(&mut self, id: egui::Id, text: String) {
        self.buffers.insert(id, text);
        self.errors.insert(id);
    }

    /// Whether a field is currently wearing the mark.
    pub fn is_rejected(&self, id: egui::Id) -> bool {
        self.errors.contains(&id)
    }

    /// Forget one field's in-progress edit, for when something else has just
    /// written the value it was holding a draft of -- a Reset, say. Without it
    /// the draft is committed over the new value the moment the field is left.
    pub fn forget(&mut self, id: egui::Id) {
        self.buffers.remove(&id);
        self.errors.remove(&id);
        self.editing.remove(&id);
        self.opening.remove(&id);
    }

    /// Forget every in-progress edit, for when the selection changes underneath.
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.errors.clear();
        self.editing.clear();
        self.opening.clear();
    }
}

/// Put the caret across the whole of a value field that has just been opened, so
/// that what is typed next replaces the number rather than being added to the end
/// of it.
///
/// Clicking a field that read 20 and typing 12 gave 2012, and on a count with a
/// maximum it gave whatever the maximum was -- 7 and "12" came out as 512. That
/// is not what clicking a number and typing means anywhere, and it is not what
/// the field asks for either: it is opened by a *click*, one gesture on the value
/// as a whole, and never by a caret placed anywhere in particular.
///
/// The state exists by now because the text field has already been drawn this
/// frame; if it somehow has not, the field opens with the caret at the end, which
/// is what it did before.
pub(crate) fn select_whole_value(ui: &egui::Ui, id: egui::Id, text: &str) {
    let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) else { return };
    let whole =
        egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
    state.cursor.set_char_range(Some(whole));
    egui::TextEdit::store_state(ui.ctx(), id, state);
}

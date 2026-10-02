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

impl FieldBuffers {
    /// Note a value field's place in this pass's drawing order.
    pub(super) fn drawn_in_order(&mut self, ui: &egui::Ui, id: egui::Id) {
        let pass = ui.ctx().cumulative_pass_nr();
        if pass != self.pass {
            // A field opened but not drawn since (its window closed) is never going to take the keyboard.
            self.opening.retain(|opening| self.drawn.contains(opening));
            self.pass = pass;
            self.drawn_before = std::mem::take(&mut self.drawn);
        }
        self.drawn.push(id);
    }

    /// Open the value field after `id` (before it, `backwards`) for typing. It takes the keyboard
    /// when next drawn, and the keymap holds off until then (`FieldBuffers::opening_one`).
    pub(super) fn open_neighbour(&mut self, id: egui::Id, backwards: bool) {
        let order = if self.drawn_before.contains(&id) { &self.drawn_before } else { &self.drawn };
        let Some(at) = order.iter().position(|&drawn| drawn == id) else { return };
        let next = if backwards { at.checked_sub(1) } else { Some(at + 1) };
        let Some(&next) = next.and_then(|next| order.get(next)) else { return };
        self.editing.insert(next);
        self.opening.insert(next);
    }

    /// Whether a field is about to take the keyboard, so keys typed meanwhile are its, not the keymap's.
    pub fn opening_one(&self, ctx: &egui::Context) -> bool {
        !self.opening.is_empty() && ctx.cumulative_pass_nr() <= self.pass + 1
    }
}

/// Select a just-opened value field's whole text, so typing replaces it (typing 12 into 20 gave
/// 2012).
pub(crate) fn select_whole_value(ui: &egui::Ui, id: egui::Id, text: &str) {
    let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    let whole =
        egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
    state.cursor.set_char_range(Some(whole));
    egui::TextEdit::store_state(ui.ctx(), id, state);
}

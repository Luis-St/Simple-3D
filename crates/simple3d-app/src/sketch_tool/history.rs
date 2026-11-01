//! The editor's own undo: stepping back and forth, and changes from one popup field in a row kept as
//! one step.

use super::*;

impl SketchTool {
    /// [`SketchTool::remember`] for a change from popup field `field`: changes from the same field
    /// one after another, as a drag on it makes every frame, are one undo step.
    pub(crate) fn remember_field(&mut self, before: Sketch, field: &str) {
        if self.field_step.as_deref() == Some(field) {
            self.redo.clear();
            return;
        }
        self.remember(before);
        self.field_step = Some(field.to_string());
    }

    pub(crate) fn undo_step(&mut self) {
        if let Some(before) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.sketch, before));
            self.clicks.clear();
            self.refresh();
        }
    }

    pub(crate) fn redo_step(&mut self) {
        if let Some(after) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.sketch, after));
            self.clicks.clear();
            self.refresh();
        }
    }
}

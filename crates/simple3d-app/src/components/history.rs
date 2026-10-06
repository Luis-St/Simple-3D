//! Undo and redo, aware that some steps made a component.
//!
//! Making a component is a step in the history of the component the group was in, while the new
//! component lives beside it. Undoing the step removes the component, and redo restores it as the
//! undo left it.

use super::*;
use crate::app::{App, Modal, Status};

impl App {
    /// Undo the last step (`Command::Undo`), asking first if it would discard a worked-on component.
    pub fn undo(&mut self) {
        let made = self.undo_would_remove();
        if let Some(&first) = made.first() {
            if self.components_have_work(&made) {
                self.component_ask = Some(ComponentAsk::Undo(first));
                self.modal = Modal::ConfirmComponent;
                return;
            }
        }
        self.undo_now();
    }

    /// The components the next undo would remove: the placed one first, then any inside it.
    pub(crate) fn undo_would_remove(&self) -> Vec<ComponentId> {
        self.history.undo_creates().iter().copied().filter(|&id| self.project.get(id).is_some()).collect()
    }

    /// Whether components `made` hold work an undo would lose: edits in their tabs, or integrations in
    /// components the undo leaves. Integrations among themselves and in the on-screen component (the
    /// step's own) do not count.
    pub(crate) fn components_have_work(&self, made: &[ComponentId]) -> bool {
        let edited = made.iter().any(|&id| self.project.get(id).is_some_and(|c| c.model.history.revision() != 0));
        let active = self.project.active;
        let used_elsewhere = self
            .project
            .components
            .iter()
            .filter(|c| c.id != active && !made.contains(&c.id))
            .any(|c| made.iter().any(|&id| !c.model.scene.integrations_of(id).is_empty()));
        edited || used_elsewhere
    }

    /// Undo the last step, whatever it discards.
    pub fn undo_now(&mut self) {
        // The round tool's draft is not in the history, so the tool is put away first (issue 88).
        self.cancel_round_tool();
        let made = self.undo_would_remove();
        match self.history.undo(&mut self.scene) {
            Some(label) => {
                self.retire_components(&made);
                self.after_history(&format!("Undid {label}"));
            }
            None => self.status = Status::Info("Nothing to undo".into()),
        }
    }

    /// Redo the last undone step (`Command::Redo`), with any components it made.
    pub fn redo(&mut self) {
        self.cancel_round_tool();
        let made = self.history.redo_creates().to_vec();
        match self.history.redo(&mut self.scene) {
            Some(label) => {
                for component in made {
                    self.revive_component(component);
                }
                self.after_history(&format!("Redid {label}"));
            }
            None => self.status = Status::Info("Nothing to redo".into()),
        }
    }

    /// Remove components `ids` for an undo, keeping them for redo. All leave before any integration is
    /// stripped, since one may hold another.
    fn retire_components(&mut self, ids: &[ComponentId]) {
        let mut retired = Vec::new();
        for &id in ids {
            let Some(at) = self.project.index(id) else { continue };
            retired.push(self.project.components.remove(at));
            self.project.open.retain(|&open| open != id);
        }
        if retired.is_empty() {
            return;
        }
        for component in &retired {
            self.strip_integrations(component.id, "Undo making a component");
        }
        self.project.removed.extend(retired);
        self.project.structure_revision += 1;
        self.relink_components();
    }

    /// Bring back component `id`, which an undo removed.
    fn revive_component(&mut self, id: ComponentId) {
        let Some(at) = self.project.removed.iter().position(|c| c.id == id) else { return };
        let component = self.project.removed.remove(at);
        self.project.components.push(component);
        self.project.structure_revision += 1;
        self.relink_components();
    }

    /// Carry out the component question answered yes.
    pub fn confirm_component_ask(&mut self) {
        let ask = self.component_ask.take();
        self.modal = Modal::None;
        match ask {
            Some(ComponentAsk::Undo(_)) => self.undo_now(),
            Some(ComponentAsk::Delete(id)) => self.delete_component(id),
            None => {}
        }
    }

    pub fn cancel_component_ask(&mut self) {
        self.component_ask = None;
        self.modal = Modal::None;
    }
}

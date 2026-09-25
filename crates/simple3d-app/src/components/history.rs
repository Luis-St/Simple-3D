//! Undo and redo, which know that some steps made a component.
//!
//! Every component has a history of its own, and a component is not part of
//! any one of them: making one is a step in the history of the component the
//! group was in, and the component it made lives beside it. So taking that
//! step back takes the component away as well, and putting it back brings the
//! component back -- as it was when the undo took it, which is kept for exactly
//! that.

use super::*;
use crate::app::{App, Modal, Status};

impl App {
    /// Take the last step back (`Command::Undo`), asking first when that would
    /// throw away a component that has been worked on since it was made.
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

    /// The components the next undo would take away with it: the one the step
    /// placed first, and any it brought along inside it after that.
    pub(crate) fn undo_would_remove(&self) -> Vec<ComponentId> {
        self.history.undo_creates().iter().copied().filter(|&id| self.project.get(id).is_some()).collect()
    }

    /// Whether the components `made` hold anything an undo of their making
    /// would lose: an edit made in one of their own tabs, or an integration of
    /// one of them in a component the undo leaves behind.
    ///
    /// The ones they hold one another in do not count: those go with the undo,
    /// and a saved primitive that carried a component inside it is still
    /// untouched when it has just been placed. Nor do the integrations in the
    /// component on screen: the step that made them is its last one -- making a
    /// component closes the step -- so every one of them is that step's own.
    pub(crate) fn components_have_work(&self, made: &[ComponentId]) -> bool {
        let edited = made.iter().any(|&id| self.project.get(id).is_some_and(|c| c.history.revision() != 0));
        let active = self.project.active;
        let used_elsewhere = self
            .project
            .components
            .iter()
            .filter(|c| c.id != active && !made.contains(&c.id))
            .any(|c| made.iter().any(|&id| !c.scene.integrations_of(id).is_empty()));
        edited || used_elsewhere
    }

    /// Take the last step back, whatever it throws away.
    pub fn undo_now(&mut self) {
        let made = self.undo_would_remove();
        match self.history.undo(&mut self.scene) {
            Some(label) => {
                self.retire_components(&made);
                self.after_history(&format!("Undid {label}"));
            }
            None => self.status = Status::Info("Nothing to undo".into()),
        }
    }

    /// Put the last step taken back back (`Command::Redo`), with the
    /// components it made if it made any.
    pub fn redo(&mut self) {
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

    /// Take the components `ids` out of the project for an undo, keeping them
    /// for a redo to bring back.
    ///
    /// All of them leave the project before any integration is stripped: one
    /// of them may hold another, and stripping it there would empty it of what
    /// a redo has to bring back.
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

    /// Bring back component `id`, which an undo took away.
    fn revive_component(&mut self, id: ComponentId) {
        let Some(at) = self.project.removed.iter().position(|c| c.id == id) else { return };
        let component = self.project.removed.remove(at);
        self.project.components.push(component);
        self.project.structure_revision += 1;
        self.relink_components();
    }

    /// Carry out the component question the dialog was answered yes to.
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

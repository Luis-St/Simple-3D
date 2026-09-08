//! Closing a tab, and the question that asks first.

use super::*;
use crate::app::{App, Modal, Status};

impl App {
    /// Close the tab at `index`, asking first if it has changes that would be
    /// lost. The last tab does not close: it is emptied, so there is always a
    /// document to work in.
    pub fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        let (_, unsaved) = self.tab_summary(index);
        if unsaved {
            self.pending_close = Some(index);
            self.modal = Modal::ConfirmCloseTab;
            return;
        }
        self.close_tab_now(index);
    }

    /// Close the tab at `index` whatever state it is in. What the confirmation
    /// calls once the question has been answered.
    pub fn close_tab_now(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        let (name, _) = self.tab_summary(index);
        if self.tabs.len() == 1 {
            self.attach(Document::empty());
            self.starter_scene();
            self.status = Status::Info(format!("Closed {name}"));
            return;
        }
        if index == self.active {
            // Show the tab to the right, or the one to the left if this was the
            // last: the neighbour, either way, rather than jumping to an end.
            let next = if index + 1 < self.tabs.len() { index + 1 } else { index - 1 };
            let doc = std::mem::replace(&mut self.tabs[next], Document::empty());
            self.tabs.remove(index);
            self.active = if next > index { next - 1 } else { next };
            self.attach(doc);
        } else {
            self.tabs.remove(index);
            if index < self.active {
                self.active -= 1;
            }
        }
        self.status = Status::Info(format!("Closed {name}"));
    }

    /// Close the tab the confirmation was asked about, discarding its changes.
    pub fn confirm_close_tab(&mut self) {
        if let Some(index) = self.pending_close.take() {
            self.close_tab_now(index);
        }
        self.modal = Modal::None;
    }

    /// Save the tab the confirmation was asked about, then close it if the save
    /// went through. Only the active tab can be saved -- saving asks for a path
    /// and writes what the editor is showing -- so it is shown first.
    pub fn save_and_close_tab(&mut self) {
        let Some(index) = self.pending_close.take() else {
            self.modal = Modal::None;
            return;
        };
        self.modal = Modal::None;
        self.activate_tab(index);
        self.save();
        if !self.unsaved() {
            self.close_tab_now(self.active);
        }
    }

    pub fn cancel_close_tab(&mut self) {
        self.pending_close = None;
        self.modal = Modal::None;
    }
}

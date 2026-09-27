//! Closing a tab, and the question that asks first.

use super::*;
use crate::app::{App, Modal, Status};

impl App {
    /// Close the tab at `index`, asking first if changes would be lost. The last tab is emptied instead,
    /// so there is always a document.
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

    /// Close the tab regardless; what the confirmation calls.
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
            // Show the neighbour, right if there is one, else left.
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

    /// Close the asked-about tab, discarding its changes.
    pub fn confirm_close_tab(&mut self) {
        if let Some(index) = self.pending_close.take() {
            self.close_tab_now(index);
        }
        self.modal = Modal::None;
    }

    /// Save the asked-about tab, closing it if the save went through. Only the active tab can be saved,
    /// so it is shown first.
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

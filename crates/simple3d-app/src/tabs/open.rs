//! Opening a document in a tab, and starting a new one.

use super::*;
use crate::app::{App, Status};
use simple3d_core::config::OpenTarget;
use std::path::Path;

impl App {
    /// Show the tab at `index`, putting the current document away first.
    pub fn activate_tab(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.active {
            return;
        }
        let current = self.detach();
        self.tabs[self.active] = current;
        let next = std::mem::replace(&mut self.tabs[index], Document::empty());
        self.active = index;
        self.attach(next);
        let (name, _) = self.tab_summary(index);
        self.status = Status::Info(format!("Showing {name}"));
    }

    /// Move `delta` tabs along, wrapping at both ends so one key can walk the
    /// whole row.
    pub fn cycle_tab(&mut self, delta: isize) {
        let count = self.tabs.len() as isize;
        if count < 2 {
            return;
        }
        let next = (self.active as isize + delta).rem_euclid(count) as usize;
        self.activate_tab(next);
    }

    /// Open `doc` in a tab of its own, after the current one, and show it.
    pub(super) fn open_tab(&mut self, doc: Document) {
        let current = self.detach();
        self.tabs[self.active] = current;
        let at = self.active + 1;
        self.tabs.insert(at, Document::empty());
        self.active = at;
        self.attach(doc);
    }

    /// A new, empty document in a new tab (`Command::New`).
    pub fn new_project(&mut self) {
        self.open_tab(Document::empty());
        self.starter_scene();
        self.status = Status::Info("New project".into());
    }

    /// Whether the current document is one nothing has been done to: an empty,
    /// unsaved, never-saved document is scratch space, and opening a file uses
    /// it rather than leaving an empty tab behind.
    pub(crate) fn active_is_scratch(&self) -> bool {
        self.path.is_none() && !self.unsaved() && self.scene.node(self.scene.root()).children.is_empty()
    }

    /// The tab `path` is already open in, if it is open at all.
    pub(crate) fn tab_for_path(&self, path: &Path) -> Option<usize> {
        (0..self.tabs.len()).find(|index| {
            let held = if *index == self.active { self.path.as_deref() } else { self.tabs[*index].path.as_deref() };
            held == Some(path)
        })
    }

    /// Open a project file, wherever the settings say a model opened while the
    /// application is already running should go (issue 107).
    ///
    /// A file that is already open is shown rather than read a second time,
    /// whichever window it is open in -- the shell answers that half, since a
    /// window knows nothing about the others' documents. An untouched,
    /// never-saved document is scratch space and is opened into rather than
    /// left behind, which is why choosing "a window of its own" does not put a
    /// second, empty window on screen when the first one has nothing in it.
    pub fn open_path(&mut self, path: &Path) {
        if let Some(index) = self.tab_for_path(path) {
            self.activate_tab(index);
            self.status = Status::Info(format!("{} is already open", document_name(Some(path))));
            return;
        }
        if self.settings.open_target == OpenTarget::Window && !self.active_is_scratch() {
            self.window_request = Some(crate::shell::WindowRequest::Open(path.to_path_buf()));
            return;
        }
        self.open_path_in_tab(path);
    }

    /// Open a project file in this window: in the current tab if that is still
    /// scratch space, and otherwise in a tab of its own. What the shell falls
    /// back to when a window of its own cannot be had.
    pub(crate) fn open_path_in_tab(&mut self, path: &Path) {
        if !self.active_is_scratch() {
            self.open_tab(Document::empty());
        }
        self.load_into_active(path);
    }
}

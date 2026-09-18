//! Taking a document out of a window and putting one in (issue 107).
//!
//! The invariant the tab switching rests on holds here too: the entry at
//! `App::active` is a stand-in and the live document is the one on `App`, so
//! everything below starts by putting the live one away and ends by bringing one
//! back out. A window is never left without a document -- one that has just had
//! its last one taken is holding an empty one, and the shell closes it in the
//! same breath.

use super::*;
use crate::app::App;

impl App {
    /// Take the document in the tab at `index` out of this window.
    ///
    /// What is left on screen afterwards is the neighbour, exactly as it is when
    /// a tab is closed: the one to the right, or the one to the left if the tab
    /// taken was the last in the row.
    pub(crate) fn take_tab(&mut self, index: usize) -> Option<Document> {
        if index >= self.tabs.len() {
            return None;
        }
        let current = self.detach();
        self.tabs[self.active] = current;
        let taken = self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.tabs.push(Document::empty());
            self.active = 0;
            self.attach(Document::empty());
            return Some(taken);
        }
        self.active = if index < self.active {
            self.active - 1
        } else if index == self.active {
            index.min(self.tabs.len() - 1)
        } else {
            self.active
        };
        let showing = std::mem::replace(&mut self.tabs[self.active], Document::empty());
        self.attach(showing);
        Some(taken)
    }

    /// Take every document out of this window, in the order they are in the row,
    /// and leave it holding an empty one. Only ever called on a window that is
    /// about to be closed.
    pub(crate) fn take_all_tabs(&mut self) -> Vec<Document> {
        let current = self.detach();
        self.tabs[self.active] = current;
        let taken = std::mem::take(&mut self.tabs);
        self.tabs = vec![Document::empty()];
        self.active = 0;
        self.attach(Document::empty());
        taken
    }

    /// Show `doc` in this window, writing over the document on screen if that
    /// is scratch space.
    ///
    /// Only for a window that has just been made for this document: an
    /// untouched, never-saved document is scratch space, and a window opened to
    /// hold one document must not end up with two tabs.
    pub(crate) fn adopt(&mut self, doc: Document) {
        if self.tabs.len() == 1 && self.active_is_scratch() {
            self.attach(doc);
            return;
        }
        self.open_tab(doc);
    }

    /// Take `doc` into this window, in a tab of its own, always.
    ///
    /// What a document moved from another window gets, and it is deliberately
    /// not [`App::adopt`]: writing over the scratch document made the move
    /// invisible. A tab was dragged onto this window's row, the window it came
    /// from closed behind it, and this window went on showing one tab -- which
    /// reads as the document having been thrown away rather than moved (issue
    /// 107). An empty tab left beside it is a far smaller sin than a move
    /// nobody can see, and it is one click to close.
    pub(crate) fn receive(&mut self, doc: Document) {
        self.open_tab(doc);
    }

    /// What this window is called where another window offers to send a document
    /// to it: the document on screen, and how many more are behind it.
    pub(crate) fn window_summary(&self) -> String {
        let (name, _) = self.tab_summary(self.active);
        match self.tab_count() {
            0 | 1 => name,
            2 => format!("{name} and 1 more"),
            n => format!("{name} and {} more", n - 1),
        }
    }
}

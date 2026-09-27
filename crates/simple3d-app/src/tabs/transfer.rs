//! Taking a document out of a window and putting one in (issue 107).
//!
//! As with tab switching, the live document is on `App` and its tab entry a stand-in, so each
//! operation puts the live one away first and brings one back after. A window always holds a
//! document; one emptied here holds a blank and is closed by the shell.

use super::*;
use crate::app::App;

impl App {
    /// Take the document in tab `index` out of this window, leaving its neighbour on screen as when a
    /// tab closes.
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

    /// Take every document out in row order, leaving a blank; only for a window about to close.
    pub(crate) fn take_all_tabs(&mut self) -> Vec<Document> {
        let current = self.detach();
        self.tabs[self.active] = current;
        let taken = std::mem::take(&mut self.tabs);
        self.tabs = vec![Document::empty()];
        self.active = 0;
        self.attach(Document::empty());
        taken
    }

    /// Show `doc`, overwriting a scratch document; only for a window just made for it, so it gets one tab.
    pub(crate) fn adopt(&mut self, doc: Document) {
        if self.tabs.len() == 1 && self.active_is_scratch() {
            self.attach(doc);
            return;
        }
        self.open_tab(doc);
    }

    /// Take `doc` in its own tab, always, for documents moved from another window: overwriting the
    /// scratch document made the move look like data loss (issue 107).
    pub(crate) fn receive(&mut self, doc: Document) {
        self.open_tab(doc);
    }

    /// This window's name in other windows' send menus: the on-screen document and how many more.
    pub(crate) fn window_summary(&self) -> String {
        let (name, _) = self.tab_summary(self.active);
        match self.tab_count() {
            0 | 1 => name,
            2 => format!("{name} and 1 more"),
            n => format!("{name} and {} more", n - 1),
        }
    }
}

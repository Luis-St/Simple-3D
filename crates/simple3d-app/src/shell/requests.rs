//! Carrying out window requests once every window has drawn, since moving documents or closing
//! windows needs several windows mutably.

use super::*;
use crate::app::Status;
use crate::tabs::document_name;
use std::path::Path;

impl Shell {
    pub(super) fn resolve_requests(&mut self, ctx: &egui::Context) {
        let asked: Vec<(u64, WindowRequest)> = self
            .windows
            .iter_mut()
            .filter_map(|window| window.window_request.take().map(|request| (window.window_id, request)))
            .collect();
        for (from, request) in asked {
            // An earlier request this frame may have closed the asking window.
            let Some(index) = self.index_of(from) else { continue };
            match request {
                WindowRequest::Open(path) => self.open_in_window(ctx, index, &path),
                WindowRequest::Detach(tab) => self.detach_tab(ctx, index, tab),
                WindowRequest::MoveTab(tab, to) => self.move_tabs(ctx, index, Some(tab), to),
                WindowRequest::MoveAll(to) => self.move_tabs(ctx, index, None, to),
                WindowRequest::Offer(tab) => self.offer = Some(Offer { from, tab, since: std::time::Instant::now() }),
                WindowRequest::Close => self.close_window(ctx, index),
                WindowRequest::Quit => self.quit(ctx),
            }
        }
    }

    /// Open a file in its own window, or show the window already holding it, so no file is open twice.
    fn open_in_window(&mut self, ctx: &egui::Context, from: usize, path: &Path) {
        let held = self
            .windows
            .iter()
            .enumerate()
            .find_map(|(index, window)| window.tab_for_path(path).map(|tab| (index, tab)));
        if let Some((index, tab)) = held {
            self.windows[index].activate_tab(tab);
            self.windows[index].status = Status::Info(format!("{} is already open", document_name(Some(path))));
            self.focus(ctx, index);
            return;
        }
        // No windows possible: open in a tab of the asking window and say so.
        if !self.can_open_windows() {
            self.windows[from].open_path_in_tab(path);
            self.windows[from].status = Status::Warning(
                "Opened in a tab: dialogs are drawn inside the window, so there can be only one".into(),
            );
            return;
        }
        let window = self.add_window(ctx);
        self.windows[window].load_into_active(path);
        self.focus(ctx, window);
    }

    /// Whether a second window is possible; embedded dialogs leave no viewports
    /// (`Shell::fold_windows_together`).
    fn can_open_windows(&self) -> bool {
        !self.settings.embed_dialogs
    }

    /// Put one tab into its own window; a window's only tab is left alone.
    fn detach_tab(&mut self, ctx: &egui::Context, index: usize, tab: usize) {
        if self.windows[index].tab_count() < 2 {
            self.windows[index].status = Status::Info("This is the window's only document".into());
            return;
        }
        if !self.can_open_windows() {
            self.windows[index].status =
                Status::Warning("Dialogs are drawn inside the window, so there can be only one window".into());
            return;
        }
        let Some(document) = self.windows[index].take_tab(tab) else { return };
        let window = self.add_window(ctx);
        self.windows[window].adopt(document);
        self.focus(ctx, window);
    }

    /// Move one tab or every tab into another window. Moving the last tab closes the source window,
    /// which makes dragging a single-tab window's tab a merge.
    fn move_tabs(&mut self, ctx: &egui::Context, index: usize, tab: Option<usize>, to: u64) {
        let Some(target) = self.index_of(to) else { return };
        if target == index {
            return;
        }
        let whole = tab.is_none() || self.windows[index].tab_count() < 2;
        let documents = match tab.filter(|_| !whole) {
            Some(tab) => self.windows[index].take_tab(tab).into_iter().collect(),
            None => self.windows[index].take_all_tabs(),
        };
        let count = documents.len();
        for document in documents {
            self.windows[target].receive(document);
        }
        if whole {
            // The source now holds only a fresh scratch document, so close it without asking.
            self.close_window_now(index);
        }
        if let Some(target) = self.index_of(to) {
            self.windows[target].status = Status::Info(match count {
                1 => "Moved a document into this window".to_string(),
                n => format!("Moved {n} documents into this window"),
            });
            self.focus(ctx, target);
        }
    }

    /// Give the held-out tab to the window whose tab row the pointer is on, or its own window once
    /// unclaimed. The source window never claims it.
    pub(super) fn resolve_offer(&mut self, ctx: &egui::Context) {
        let Some(offer) = &self.offer else { return };
        let (from, tab, since) = (offer.from, offer.tab, offer.since);
        let Some(index) = self.index_of(from) else {
            self.offer = None;
            return;
        };
        let claimed = self.windows.iter().find(|window| window.window_id != from && window.pointer_on_strip);
        if let Some(to) = claimed.map(|window| window.window_id) {
            self.offer = None;
            self.move_tabs(ctx, index, tab, to);
            return;
        }
        if since.elapsed() < CLAIM {
            // Keep frames coming while the tab is held, or the target window never notices.
            ctx.request_repaint();
            return;
        }
        self.offer = None;
        // Unclaimed: a tab becomes its own window; a whole window stays as it is.
        if let Some(tab) = tab {
            self.detach_tab(ctx, index, tab);
        }
    }

    /// Close a window the user asked to close; its unsaved documents were already asked about.
    fn close_window(&mut self, ctx: &egui::Context, index: usize) {
        if self.windows.len() == 1 {
            self.quit(ctx);
            return;
        }
        self.close_window_now(index);
    }

    /// Remove a window; dropping its job channel stops its worker. The next first window takes the
    /// root viewport (see the module docs).
    fn close_window_now(&mut self, index: usize) {
        self.windows.remove(index);
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.windows[0].persist();
        // The root viewport's window must let the following close through.
        self.windows[0].leaving = true;
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
    }

    /// Bring a window to the front; Wayland ignores this.
    fn focus(&self, ctx: &egui::Context, index: usize) {
        ctx.send_viewport_cmd_to(self.viewport_of(index), egui::ViewportCommand::Focus);
    }
}

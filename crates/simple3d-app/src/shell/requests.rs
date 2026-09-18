//! Carrying out what the windows asked for, once they have all drawn.
//!
//! Nothing here runs while a window is being drawn, which is the whole point of
//! going through [`WindowRequest`]: moving a document between two windows takes
//! both of them mutably, and closing one moves every window after it in the
//! vector.

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
            // The window that asked may already be gone: an earlier request in
            // the same frame can have closed it.
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

    /// Open a file in a window of its own -- unless one of the windows already
    /// holds it, in which case that window is shown instead. Reading one file
    /// into two windows would be two documents that disagree and one of them
    /// silently losing, which is the same reason a file already open in a tab is
    /// never opened twice.
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
        // Nowhere to put a window: the file opens in a tab of the window that
        // asked, which is what the setting's other answer does, and says so
        // rather than quietly doing something else.
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

    /// Whether a second window can be had at all. With dialogs drawn inside the
    /// window there are no viewports to put one in -- see
    /// `Shell::fold_windows_together`.
    fn can_open_windows(&self) -> bool {
        !self.settings.embed_dialogs
    }

    /// Put one tab into a window of its own. A window's only tab is already in a
    /// window of its own, so that gesture does nothing rather than opening an
    /// empty window beside it.
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

    /// Move one tab, or every tab, into another window.
    ///
    /// Moving the last tab out of a window moves the window: what would be left
    /// is an empty window nobody asked for, so it goes -- which is also what
    /// makes dragging a single-tab window's tab onto another window read as
    /// merging the two.
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
            // The source window is empty now -- `take_all_tabs` leaves it
            // holding a fresh scratch document -- so it is closed without
            // asking: nothing was in it that has not just been moved.
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

    /// Give the tab that is being held out to the window the pointer is over,
    /// or, once nobody has taken it, to a window of its own.
    ///
    /// Only one window can claim it: the one whose own row of tabs the pointer
    /// is on this frame. The window it came from is not asked -- the pointer was
    /// outside it when the button came up, which is what put the tab in the air
    /// in the first place.
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
            // Frames have to keep coming while the tab is in the air, or the
            // window under the pointer never draws the frame it would notice
            // in -- nothing else is asking for one, since the drag is over.
            ctx.request_repaint();
            return;
        }
        self.offer = None;
        // Nobody took it, so a tab becomes a window of its own -- which is what
        // letting one go outside the window asks for. A whole window let go over
        // nothing stays where it is: it is already a window of its own.
        if let Some(tab) = tab {
            self.detach_tab(ctx, index, tab);
        }
    }

    /// Close a window the user asked to close. Whatever had to be asked about
    /// its unsaved documents has been asked by the window itself.
    fn close_window(&mut self, ctx: &egui::Context, index: usize) {
        if self.windows.len() == 1 {
            self.quit(ctx);
            return;
        }
        self.close_window_now(index);
    }

    /// Take a window out of the shell. Its evaluation worker goes with it: the
    /// thread's job channel is dropped here, which is how it learns to stop.
    ///
    /// Whichever window is first afterwards is drawn in the root viewport -- see
    /// the module comment on why the root window cannot be the one that
    /// disappears.
    fn close_window_now(&mut self, index: usize) {
        self.windows.remove(index);
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.windows[0].persist();
        // The window drawn in the root viewport must not question the close
        // that follows: it is this one.
        self.windows[0].leaving = true;
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
    }

    /// Bring a window to the front, so a document that has just been moved into
    /// it, or opened in it, is the window the user is looking at. Wayland grants
    /// no client its own focus and ignores this without complaint, which is the
    /// same treatment it gives a dialog asking to stay above its parent.
    fn focus(&self, ctx: &egui::Context, index: usize) {
        ctx.send_viewport_cmd_to(self.viewport_of(index), egui::ViewportCommand::Focus);
    }
}

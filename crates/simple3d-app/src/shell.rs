//! Several windows at once, and what moves between them (issue 107).
//!
//! Each window is its own `App` with its own tabs, worker and history. `Shell` owns them in order:
//! the first draws into eframe's root viewport, the rest are immediate viewports (deferred ones
//! need a `'static` callback and could not take `&mut App`), as dialogs are.
//!
//! A window never acts on another: it leaves a [`WindowRequest`] that the shell carries out after
//! every window has drawn. Closing the root viewport ends the process, so windows are removed from
//! the vector and whichever is first takes over the root viewport.

mod frame;
mod requests;
#[cfg(test)]
mod tests;

use crate::app::App;
use std::path::PathBuf;

/// What a window asks the shell to do after the frame; at most one per window per frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowRequest {
    /// Open this file in its own window, or show the window that already has it.
    Open(PathBuf),
    /// Put the tab at this index into its own window.
    Detach(usize),
    /// Move the tab at this index into the window with this id.
    MoveTab(usize, u64),
    /// Move every tab of this window into the window with this id.
    MoveAll(u64),
    /// Released outside this window where window positions are unknown: hold the tab out for the
    /// window the pointer reaches. `None` is all of the window's tabs. See [`Shell::resolve_offer`].
    Offer(Option<usize>),
    /// Close this window; the application ends when the last one goes.
    Close,
    /// End the application, however many windows are open.
    Quit,
}

/// Another window as seen by the one being drawn: its menu name and tab row position. `strip` is
/// in screen coordinates, `None` where unknown (Wayland), where the menu is the way to move tabs.
#[derive(Clone, Debug)]
pub struct OtherWindow {
    pub id: u64,
    pub name: String,
    pub strip: Option<egui::Rect>,
}

/// A tab released outside its window, waiting to be claimed. On Wayland the drop target is
/// unknown, so the window the pointer turns up over claims it; unclaimed, it becomes a window.
struct Offer {
    from: u64,
    /// Which tab, or every tab of the window.
    tab: Option<usize>,
    since: std::time::Instant,
}

/// How long a tab is held out: long enough for the window under the pointer to notice, short
/// enough not to be felt.
const CLAIM: std::time::Duration = std::time::Duration::from_millis(350);

/// Every window the application has open.
pub struct Shell {
    windows: Vec<App>,
    /// The next window's id; never reused, so a viewport id never names a closed window.
    next_id: u64,
    gl: Option<std::sync::Arc<eframe::glow::Context>>,
    /// The tab held out for another window to claim, if any.
    offer: Option<Offer>,
    /// The settings as last seen, so a change in one window reaches the others; without it a
    /// second window's stale copy would overwrite the first's changes.
    settings: simple3d_core::config::AppSettings,
}

impl Shell {
    pub fn new(ctx: &egui::Context, gl: Option<std::sync::Arc<eframe::glow::Context>>, open: Option<PathBuf>) -> Shell {
        let first = App::new(ctx, gl.clone(), open);
        let settings = first.settings.clone();
        Shell { windows: vec![first], next_id: 1, gl, offer: None, settings }
    }

    /// An empty new window; the caller fills it.
    fn add_window(&mut self, ctx: &egui::Context) -> usize {
        let mut app = App::with_config_dir(ctx, None, self.windows[0].config_dir().to_path_buf());
        // eframe shares one OpenGL context across viewports, so every window can use the GPU renderer.
        app.gl = self.gl.clone();
        app.window_id = self.next_id;
        self.next_id += 1;
        app.settings = self.settings.clone();
        self.windows.push(app);
        self.windows.len() - 1
    }

    /// A window's viewport: the root one for the first window, the shell's own for the rest.
    fn viewport_of(&self, index: usize) -> egui::ViewportId {
        if index == 0 {
            egui::ViewportId::ROOT
        } else {
            egui::ViewportId::from_hash_of(("simple3d-window", self.windows[index].window_id))
        }
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        self.windows.iter().position(|window| window.window_id == id)
    }
}

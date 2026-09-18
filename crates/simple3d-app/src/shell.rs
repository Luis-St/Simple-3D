//! Several windows at once, and what moves between them (issue 107).
//!
//! One window was the whole application until now: `App` was what eframe drove,
//! and its tabs were the only way to have two models open. A tab is the right
//! answer most of the time and is still the default, but it cannot put two
//! models side by side on one screen, or one on each display -- so a document
//! can now be pulled out into a window of its own, pushed back into another
//! window, and opened into one straight away (`OpenTarget::Window`).
//!
//! The shape of it: every window is an `App` of its own, with its own tabs, its
//! own evaluation worker and its own undo history, and the whole of the rest of
//! the application goes on knowing nothing about how many there are. `Shell`
//! owns them in the order they are shown; the first draws into the root viewport
//! eframe made, and the rest are `show_viewport_immediate` windows built in the
//! same pass, exactly as a dialog is (see `app_chrome::dialog`). Immediate
//! rather than deferred for the same reason: a deferred viewport takes a
//! `'static` callback and could not be handed `&mut App`.
//!
//! The one thing a window may not do is act on another window itself. A window
//! that wants a tab moved, a window opened or itself closed leaves a
//! [`WindowRequest`] on its `App`, and the shell carries it out after every
//! window has drawn -- so no window is ever half-drawn while the vector it lives
//! in is being rearranged underneath it.
//!
//! The root viewport is the one asymmetry, and it is the window system's rather
//! than ours: closing it ends the process, so it cannot be the window that
//! disappears. Windows are therefore removed by taking them out of the vector,
//! and whichever window is first afterwards draws into the root viewport. When
//! the *first* window is the one that goes, the next one takes the root window
//! over -- the tabs are all where the user put them, but the window that stays
//! on screen is the one the desktop already had.

mod frame;
mod requests;
#[cfg(test)]
mod tests;

use crate::app::App;
use std::path::PathBuf;

/// What a window asks the shell to do once the frame it asked in is over.
///
/// One per window per frame: everything here is the answer to a gesture or a
/// menu entry, and a frame holds at most one of those.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowRequest {
    /// Open this file in a window of its own -- or, if it is already open in one
    /// of the windows, show it there rather than twice.
    Open(PathBuf),
    /// Put the tab at this index into a window of its own.
    Detach(usize),
    /// Move the tab at this index into the window with this id.
    MoveTab(usize, u64),
    /// Move every tab of this window into the window with this id.
    MoveAll(u64),
    /// Let go outside this window, on a desktop that does not say where windows
    /// are: hold the tab out for whichever window the pointer turns up over.
    /// `None` is the whole window's tabs. See [`Shell::resolve_offer`].
    Offer(Option<usize>),
    /// Close this window. The application ends when the last one goes.
    Close,
    /// End the application, however many windows are open.
    Quit,
}

/// One of the other windows, as the window being drawn is told about it: what to
/// call it in a menu, and where its row of tabs is on the desktop.
///
/// `strip` is in screen coordinates and is `None` wherever the window system
/// does not say where a window is. Wayland is that case and is not an edge
/// case: a client there cannot ask for its own position, so a tab cannot be
/// dropped onto a window it cannot locate and the menu is the way to move one.
#[derive(Clone, Debug)]
pub struct OtherWindow {
    pub id: u64,
    pub name: String,
    pub strip: Option<egui::Rect>,
}

/// A tab let go outside the window it came from, waiting to be claimed.
///
/// It exists because of what a Wayland client is not told. The drop itself is
/// invisible to us -- no window position, and the window the tab was let go over
/// sees nothing at all while another window holds the pointer -- so instead of
/// guessing, the tab is held out for a moment: the button is up by then, the
/// pointer reaches whatever is under it, and the window it turns up over says so
/// itself. Nothing claims it on an empty desktop, and it becomes a window of its
/// own, which is what letting a tab go outside the window means anyway.
struct Offer {
    from: u64,
    /// Which tab, or every tab of the window.
    tab: Option<usize>,
    since: std::time::Instant,
}

/// How long a tab is held out for. Long enough for the window under the pointer
/// to draw the frame in which it notices, short enough that a tab let go over
/// the desktop becomes a window without the delay being felt.
const CLAIM: std::time::Duration = std::time::Duration::from_millis(350);

/// Every window the application has open.
pub struct Shell {
    windows: Vec<App>,
    /// The id the next window will wear. Never reused, so a viewport id can
    /// never be a window that has closed.
    next_id: u64,
    gl: Option<std::sync::Arc<eframe::glow::Context>>,
    /// The tab held out for another window to claim, if there is one.
    offer: Option<Offer>,
    /// The settings as they were last seen, so a change made in one window
    /// reaches the others. Each window holds its own copy -- the whole
    /// application reads `app.settings` -- and only the first window writes the
    /// file, so without this the second window's copy would go back over the
    /// first window's changes the next time anything was changed in it.
    settings: simple3d_core::config::AppSettings,
}

impl Shell {
    pub fn new(ctx: &egui::Context, gl: Option<std::sync::Arc<eframe::glow::Context>>, open: Option<PathBuf>) -> Shell {
        let first = App::new(ctx, gl.clone(), open);
        let settings = first.settings.clone();
        Shell { windows: vec![first], next_id: 1, gl, offer: None, settings }
    }

    /// A window of its own, with nothing in it. The caller fills it.
    fn add_window(&mut self, ctx: &egui::Context) -> usize {
        let mut app = App::with_config_dir(ctx, None, self.windows[0].config_dir().to_path_buf());
        // The same OpenGL context as every other window: eframe gives all its
        // viewports one, so a window opened later can use the GPU renderer as
        // readily as the first one.
        app.gl = self.gl.clone();
        app.window_id = self.next_id;
        self.next_id += 1;
        app.settings = self.settings.clone();
        self.windows.push(app);
        self.windows.len() - 1
    }

    /// The viewport a window is drawn in. The first window is the root one
    /// eframe opened; every other is a window of the shell's own making.
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

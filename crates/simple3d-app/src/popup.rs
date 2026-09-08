//! An in-place popup: a small window that floats over the viewport rather than
//! standing in front of the whole application (issue 82).
//!
//! The kind of window a CAD tool puts a command's settings in -- Fusion 360's
//! command palettes are the shape this copies. Three things separate it from
//! the dialogs in [`crate::app_chrome`]:
//!
//! * **It is drawn inside the viewport and cannot leave it.** A dialog is a
//!   window of the window system's own, which can be dragged onto the other
//!   screen and left there; this belongs to the picture it is about.
//! * **It is moved by its title bar and rolled up by the chevron on it**, so
//!   the part of the model it is covering can be seen without putting the tool
//!   away and losing what was typed into it.
//! * **It is not modal.** The viewport underneath goes on orbiting, zooming and
//!   selecting while it is open, which is the whole reason a tool with a live
//!   preview is worth having in the viewport at all.
//!
//! Nothing here knows what a split is. A popup is a title, a body, a row of
//! buttons and a [`Placement`] the application keeps for it, so the next tool
//! that wants one asks for one.

mod show;
pub use show::{action_row, body_room, show};
mod title_bar;
pub(crate) use title_bar::*;
mod settle;
pub(crate) use settle::*;
#[cfg(test)]
mod tests;

/// Where a popup sits and whether it is rolled up. Kept by the application, one
/// per popup, so a tool closed and opened again comes back where it was left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    /// The top-left corner, in screen points. `None` until the first frame,
    /// which puts it in the corner of the viewport it opens over.
    pub pos: Option<egui::Pos2>,
    /// Rolled up to its title bar alone.
    pub collapsed: bool,
    /// How tall the window came out last frame, which is what this frame keeps
    /// inside the viewport.
    ///
    /// One frame behind, and deliberately: a window's height is not known until
    /// it has been laid out, and the alternative -- letting the toolkit
    /// constrain the area itself -- moves the window without saying so. That
    /// was the bug: a tall window near the bottom edge was quietly lifted to
    /// fit, and rolling it up removed the reason for the lift, so the title bar
    /// dropped back down under the pointer that had just clicked it.
    height: f32,
}

pub struct PopupSpec<'a> {
    /// Identifies the popup to the toolkit and to the placement store. Stable
    /// across openings -- it is what remembers where the window was dragged to.
    pub key: &'static str,
    pub title: &'a str,
    /// How wide the window is. A popup is a column of fields, not a document:
    /// it has one width and takes whatever height its contents come to.
    pub width: f32,
}

/// What the user did to the window itself, as opposed to what they did in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupEvent {
    Nothing,
    /// The close cross was pressed. The caller decides what closing means --
    /// for a tool it is a cancel.
    Closed,
}

/// The height of the bar the window is dragged by.
const TITLE_BAR: f32 = 26.0;

/// The margin inside the window, around the body and the button row.
const PAD: f32 = 10.0;

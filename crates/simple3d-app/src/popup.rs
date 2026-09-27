//! An in-place popup: a small window floating over the viewport (issue 82), like Fusion 360's
//! command palettes.
//!
//! Unlike the dialogs in [`crate::app_chrome`], it stays inside the viewport, is moved by its title
//! bar and rolled up by its chevron, and is not modal, so the viewport stays interactive under a
//! live preview. It knows nothing of specific tools: a title, a body, a button row, fields and a
//! [`Placement`] the application keeps.

mod fields;
pub(crate) use fields::*;
mod show;
pub use show::{action_row, scrolling_body, show};
mod title_bar;
pub(crate) use title_bar::*;
mod settle;
pub(crate) use settle::*;
#[cfg(test)]
mod tests;

/// Where a popup sits and whether it is rolled up, kept per popup so a reopened tool returns there.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    /// The top-left corner in screen points; `None` until the first frame places it in the viewport corner.
    pub pos: Option<egui::Pos2>,
    /// Rolled up to its title bar alone.
    pub collapsed: bool,
    /// Last frame's height, used to keep this frame inside the viewport. One frame behind deliberately:
    /// letting the toolkit constrain the area moved it silently, and rolling up then dropped the title
    /// bar out from under the pointer.
    height: f32,
}

pub struct PopupSpec<'a> {
    /// Identifies the popup to the toolkit and the placement store; stable across openings.
    pub key: &'static str,
    pub title: &'a str,
    /// The window width; the height follows the contents.
    pub width: f32,
}

/// What the user did to the window itself, as opposed to in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupEvent {
    Nothing,
    /// The close cross was pressed; the caller decides what closing means (for a tool, cancel).
    Closed,
}

/// The height of the bar the window is dragged by.
const TITLE_BAR: f32 = 26.0;

/// The margin inside the window, around the body and the button row.
const PAD: f32 = 10.0;

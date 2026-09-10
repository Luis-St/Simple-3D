//! The custom pattern kind creation tool (issue 67).
//!
//! The six kinds a pattern ships with are the ones worth having a name for.
//! This is where a user makes their own: a rule built out of *stages*, each one
//! repeating whatever the stages before it made, named and kept on a shelf for
//! any other project.
//!
//! It edits the pattern node itself rather than a draft of one. A rule is only
//! ever judged by what it lays out, so every number typed here moves the copies
//! in the viewport at once, and every one of them is an ordinary parameter
//! edit -- which is why undo, the project file and the clipboard needed nothing
//! added for any of this.
//!
//! What is saved to the shelf is the *rule*, not the pattern: the stage numbers
//! and nothing else. A node still stores those numbers itself, so a project
//! opened on a machine that has never seen the kind still lays out correctly.
//!
//! It is an [in-place popup](crate::popup) rather than a dialog (issue 96). It
//! used to be a window most of the screen wide, half of it a second viewport
//! rendering the very same scene from a camera of its own -- a picture of the
//! pattern, in front of the picture of the pattern. What the tool is for is
//! seeing a rule take shape on the model, and the model is already on screen:
//! the window now floats over it, is dragged out of the way by its title bar,
//! rolls up to its bar alone, and leaves the viewport underneath free to be
//! orbited and zoomed while the numbers stay up. The preview is the viewport.
//!
//! Two of the things a rule needed most are here as well (issue 79): any of the
//! six fixed kinds can be used as the template a rule starts from, rather than
//! the tool throwing the layout away on the way in, and a stage says what it
//! *does* before it says its numbers, so a run offers a run's four fields
//! instead of all nine.

mod body;
mod kinds;
mod open;
pub(crate) use body::*;
mod stages;
pub(crate) use stages::*;
mod actions;
pub(crate) use actions::*;

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "pattern-tool";

/// How wide the window is: a stage's name and its field side by side, and no
/// wider. A popup lives over the model, so every pixel of it is a pixel of the
/// pattern being laid out that cannot be seen.
const WIDTH: f32 = 320.0;

/// The cross that drops a stage: twice the height egui's small button comes out
/// at, and square, because it is a mark rather than a word and a wide one reads
/// as a button with its label missing.
const DROP_STAGE: f32 = 28.0;

//! Which dialog is open. One at a time, so the state cannot contradict itself.

/// Which of the modal windows is open. Only one at a time, so the state cannot
/// contradict itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Modal {
    #[default]
    None,
    Export,
    Keymap,
    About,
    /// A failure worth stopping for, shown in a scrollable, copyable window.
    Error,
    /// Naming a group, or a whole project, to keep on the palette.
    SavePrimitive,
    /// Quitting with unsaved changes.
    ConfirmQuit,
    /// Emptying a collection of every piece, which turns it into a union group
    /// and lets go of the object it was made from (issue 82).
    ConfirmExtractAll,
    /// Closing a tab with unsaved changes (issue 61).
    ConfirmCloseTab,
    /// Taking a saved pattern kind off the shelf, which is a file on disk and
    /// not something undo reaches (issue 67).
    ConfirmDeleteKind,
}

//! Which dialog is open.

/// The open modal window; only one at a time, so the state cannot contradict itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Modal {
    #[default]
    None,
    Export,
    Keymap,
    About,
    /// A failure, in a scrollable, copyable window.
    Error,
    /// Naming a group or project to keep on the palette.
    SavePrimitive,
    ConfirmQuit,
    /// Emptying a collection, which makes it a union group and drops its original (issue 82).
    ConfirmExtractAll,
    /// Closing a tab with unsaved changes (issue 61).
    ConfirmCloseTab,
    /// Deleting a saved pattern kind, a file undo does not reach (issue 67).
    ConfirmDeleteKind,
    /// Deleting a component, or undoing its creation over later work (issue 113); neither can be undone.
    ConfirmComponent,
}

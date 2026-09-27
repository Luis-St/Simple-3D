//! Where a pointer gesture has got to.

/// The pointer facts the viewport reads off an `egui::Response` each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerState {
    /// Escape was pressed this frame.
    pub escape: bool,
    /// The drag button came up.
    pub released: bool,
    /// A drag started this frame.
    pub started: bool,
    /// The cursor is over a manipulator handle.
    pub on_handle: bool,
    /// There is a cursor position at all; the pointer may be off the window.
    pub have_cursor: bool,
}

/// What the pointer asks of the manipulator this frame. Separate from `panel_viewport::manipulate`
/// so the one-undo-per-drag rule (recorded only on `Begin`, acceptance criterion 23) is testable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    /// Escape during a drag: restore the pre-drag values exactly.
    Cancel,
    /// The button came up: the drag is over.
    Finish,
    /// A drag is running: follow the cursor.
    Continue,
    /// A handle was grabbed: open one undo step and start.
    Begin,
    /// Nothing for the manipulator to do.
    Idle,
}

/// The frame's phase given whether a drag is running. Escape beats release, and `Begin` requires no
/// running drag, so no second undo step opens mid-gesture.
pub fn drag_phase(dragging: bool, pointer: PointerState) -> DragPhase {
    if dragging {
        if pointer.escape {
            return DragPhase::Cancel;
        }
        if pointer.released {
            return DragPhase::Finish;
        }
        return if pointer.have_cursor { DragPhase::Continue } else { DragPhase::Idle };
    }
    if pointer.started && pointer.on_handle && pointer.have_cursor {
        DragPhase::Begin
    } else {
        DragPhase::Idle
    }
}

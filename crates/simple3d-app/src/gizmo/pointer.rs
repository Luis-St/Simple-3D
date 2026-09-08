//! Where a pointer gesture has got to.

/// The pointer facts the viewport panel reads off an `egui::Response` each frame.
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
    /// There is a cursor position at all -- the pointer may be off the window.
    pub have_cursor: bool,
}

/// What the pointer is asking the manipulator to do this frame.
///
/// Split out of `panel_viewport::manipulate` so the begin/continue/finish
/// bookkeeping can be asserted. An `egui::Response` cannot be built outside a
/// running frame, and that is what kept acceptance criterion 23's last clause --
/// "a completed drag undoes in one step" -- untested: the single undo record
/// happens on `Begin` and on no other phase, which is the whole mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    /// Escape during a drag: put the pre-drag values back exactly.
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

/// Which phase a frame is in, given whether a drag is already running.
///
/// The ordering is the part that matters. Escape beats release, so a cancel is
/// never mistaken for a completed drag; and `Begin` requires that no drag is
/// running, which is what stops a second undo step opening mid-gesture.
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

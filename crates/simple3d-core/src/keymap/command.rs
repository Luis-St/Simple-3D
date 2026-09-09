//! Every command a key can be bound to, and the part of the interface each
//! belongs to.

use serde::{Deserialize, Serialize};

/// Everything the user can bind. Adding a command here is all it takes for it to
/// appear in the keymap editor, grouped by its area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    New,
    Open,
    CloseTab,
    NextTab,
    PreviousTab,
    Save,
    SaveAs,
    Export,
    Quit,

    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    Duplicate,
    Delete,
    Group,
    Pattern,
    ConvertToMesh,
    SplitIntoPieces,
    Rejoin,
    Rename,
    ToggleVisibility,
    MoveUp,
    MoveDown,

    FrameSelection,
    FrameAll,
    ZoomToPointer,
    ViewTop,
    ViewBottom,
    ViewFront,
    ViewBack,
    ViewLeft,
    ViewRight,
    ViewIsometric,
    ToggleGrid,
    ToggleAxisX,
    ToggleAxisY,
    ToggleAxisZ,
    ToggleSection,
    DisplayShaded,
    DisplayShadedEdges,
    DisplayWireframe,
    ToggleBoundingBox,
    ToggleDocks,
    ResetLayout,

    ModeMove,
    ModeRotate,
    ModeResize,
    ModeScale,
    MeasureTool,
    SnapToGeometry,
    NudgeLeft,
    NudgeRight,
    NudgeUp,
    NudgeDown,
    NudgeAway,
    NudgeToward,
}

/// The keymap editor lists commands grouped by area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Area {
    File,
    Edit,
    View,
    Manipulate,
}

impl Area {
    pub const ALL: [Area; 4] = [Area::File, Area::Edit, Area::View, Area::Manipulate];

    pub fn label(self) -> &'static str {
        match self {
            Area::File => "File",
            Area::Edit => "Edit",
            Area::View => "View",
            Area::Manipulate => "Manipulate",
        }
    }
}

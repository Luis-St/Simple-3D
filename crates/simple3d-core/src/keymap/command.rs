//! Every bindable command, and its interface area.

use serde::{Deserialize, Serialize};

/// Everything bindable; adding one here puts it in the keymap editor under its area.
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
    Import,
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
    MakeComponent,
    NewComponent,
    ConvertToMesh,
    SimplifyMesh,
    RoundEdges,
    Reassemble,
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
    ModePushPull,
    MeasureTool,
    AlignDistribute,
    SnapToGeometry,
    NudgeLeft,
    NudgeRight,
    NudgeUp,
    NudgeDown,
    NudgeAway,
    NudgeToward,
}

/// The keymap editor's command groups.
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

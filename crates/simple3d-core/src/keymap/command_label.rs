//! What each command is called on screen.

use super::*;

impl Command {
    pub fn label(self) -> &'static str {
        use Command::*;
        match self {
            New => "New project",
            Open => "Open project",
            CloseTab => "Close the open document",
            NextTab => "Next document",
            PreviousTab => "Previous document",
            Save => "Save",
            SaveAs => "Save as",
            Export => "Export",
            Quit => "Quit",
            Undo => "Undo",
            Redo => "Redo",
            Copy => "Copy",
            Cut => "Cut",
            Paste => "Paste",
            Duplicate => "Duplicate",
            Delete => "Delete",
            Group => "Group selection",
            Pattern => "Make a pattern of the selection",
            ConvertToMesh => "Convert to a mesh",
            SplitIntoPieces => "Split into smaller pieces",
            Rejoin => "Join the pieces back together",
            Rename => "Rename",
            ToggleVisibility => "Toggle visibility",
            MoveUp => "Move up among siblings",
            MoveDown => "Move down among siblings",
            FrameSelection => "Frame selection",
            FrameAll => "Frame all",
            ViewTop => "View: top",
            ViewBottom => "View: bottom",
            ViewFront => "View: front",
            ViewBack => "View: back",
            ViewLeft => "View: left",
            ViewRight => "View: right",
            ViewIsometric => "View: isometric",
            ToggleGrid => "Show grid",
            ToggleAxisX => "Show the X axis",
            ToggleAxisY => "Show the Y axis",
            ToggleAxisZ => "Show the Z axis",
            ToggleSection => "Section view",
            DisplayShaded => "Display: shaded",
            DisplayShadedEdges => "Display: shaded with edges",
            DisplayWireframe => "Display: wireframe",
            ToggleBoundingBox => "Show bounding box",
            ToggleDocks => "Hide / show the side docks",
            ResetLayout => "Reset panel layout",
            ModeMove => "Manipulator: move",
            ModeRotate => "Manipulator: rotate",
            ModeResize => "Manipulator: resize",
            ModeScale => "Manipulator: scale",
            ToggleHandleFrame => "Handle frame: object / world",
            MeasureTool => "Measure tool",
            SnapToGeometry => "Snap to geometry (hold)",
            NudgeLeft => "Nudge left",
            NudgeRight => "Nudge right",
            NudgeUp => "Nudge up",
            NudgeDown => "Nudge down",
            NudgeAway => "Nudge along third axis (+)",
            NudgeToward => "Nudge along third axis (-)",
        }
    }

    pub fn area(self) -> Area {
        use Command::*;
        match self {
            New | Open | CloseTab | NextTab | PreviousTab | Save | SaveAs | Export | Quit => Area::File,
            Undo | Redo | Copy | Cut | Paste | Duplicate | Delete | Group | Pattern | ConvertToMesh
            | SplitIntoPieces | Rejoin | Rename | ToggleVisibility | MoveUp | MoveDown => Area::Edit,
            FrameSelection | FrameAll | ViewTop | ViewBottom | ViewFront | ViewBack | ViewLeft | ViewRight
            | ViewIsometric | ToggleGrid | ToggleAxisX | ToggleAxisY | ToggleAxisZ | ToggleSection | DisplayShaded
            | DisplayShadedEdges | DisplayWireframe | ToggleBoundingBox | ToggleDocks | ResetLayout => Area::View,
            ModeMove | ModeRotate | ModeResize | ModeScale | ToggleHandleFrame | MeasureTool | SnapToGeometry
            | NudgeLeft | NudgeRight | NudgeUp | NudgeDown | NudgeAway | NudgeToward => Area::Manipulate,
        }
    }
}

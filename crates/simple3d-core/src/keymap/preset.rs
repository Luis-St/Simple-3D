//! The bindings each preset starts from.

use super::*;
use std::collections::BTreeMap;

impl Keymap {
    pub fn from_preset(preset: Preset) -> Keymap {
        use Command::*;
        let mut bindings: BTreeMap<Command, Chord> = BTreeMap::new();
        let mut set = |command: Command, chord: Chord| {
            bindings.insert(command, chord);
        };

        set(New, Chord::ctrl("N"));
        set(Open, Chord::ctrl("O"));
        // The standard document keys of tabbed programs (issue 61).
        set(CloseTab, Chord::ctrl("W"));
        set(NextTab, Chord::ctrl("Tab"));
        set(PreviousTab, Chord::ctrl_shift("Tab"));
        set(Save, Chord::ctrl("S"));
        set(SaveAs, Chord::ctrl_shift("S"));
        // I for import, beside Export (issue 105).
        set(Import, Chord::ctrl("I"));
        set(Export, Chord::ctrl("E"));
        set(Quit, Chord::ctrl("Q"));

        set(Undo, Chord::ctrl("Z"));
        set(Redo, Chord::ctrl_shift("Z"));
        set(Copy, Chord::ctrl("C"));
        set(Cut, Chord::ctrl("X"));
        set(Paste, Chord::ctrl("V"));
        set(Duplicate, Chord::ctrl("D"));
        set(Delete, Chord::key("Delete"));
        set(Group, Chord::ctrl("G"));
        set(Pattern, Chord::ctrl_shift("P"));
        // Make component (issue 113): O for object, free with Ctrl+Shift in every preset.
        set(MakeComponent, Chord::ctrl_shift("O"));
        // New empty component (issue 113): C, free on its own in every preset.
        set(NewComponent, Chord::key("C"));
        // Bake a shape into its triangles (issue 80).
        set(ConvertToMesh, Chord::ctrl_shift("M"));
        // Simplify (issue 106): R for reduce, beside Convert; plain R is resize.
        set(SimplifyMesh, Chord::ctrl_shift("R"));
        // Round and bevel edges (issue 88): B for bevel.
        set(RoundEdges, Chord::ctrl_shift("B"));
        // Push/pull (issue 73), the fifth tool, on P in every preset.
        set(ModePushPull, Chord::key("P"));
        // Reassemble (issue 108): A for assemble, beside the conversion it undoes.
        set(Reassemble, Chord::ctrl_shift("A"));
        // Split into pieces (issue 82): K for knife.
        set(SplitIntoPieces, Chord::ctrl_shift("K"));
        // Rejoin (issue 82): J for join, one key along from K.
        set(Rejoin, Chord::ctrl_shift("J"));
        set(Rename, Chord::key("F2"));
        set(ToggleVisibility, Chord::key("H"));
        set(MoveUp, Chord::ctrl("Up"));
        set(MoveDown, Chord::ctrl("Down"));

        set(FrameSelection, Chord::key("F"));
        set(FrameAll, Chord::shift("F"));
        // Zoom to pointer while held (issue 97): Alt alone, since Ctrl snaps and Shift is coarse.
        set(ZoomToPointer, Chord::modifiers(false, false, true));
        set(ViewTop, Chord::key("7"));
        set(ViewBottom, Chord::ctrl("7"));
        set(ViewFront, Chord::key("1"));
        set(ViewBack, Chord::ctrl("1"));
        set(ViewRight, Chord::key("3"));
        set(ViewLeft, Chord::ctrl("3"));
        set(ViewIsometric, Chord::key("0"));
        set(ToggleGrid, Chord::key("5"));
        // Alt+X / Y / Z: free in every preset.
        set(ToggleAxisX, Chord::alt("X"));
        set(ToggleAxisY, Chord::alt("Y"));
        set(ToggleAxisZ, Chord::alt("Z"));
        // The last free number on the view-toggle row, beside the grid.
        set(ToggleSection, Chord::key("4"));
        set(DisplayShaded, Chord::key("8"));
        set(DisplayShadedEdges, Chord::key("9"));
        set(DisplayWireframe, Chord::key("6"));
        set(ToggleBoundingBox, Chord::key("B"));
        // Tab clears the docks, leaving the viewport alone with the model.
        set(ToggleDocks, Chord::key("Tab"));
        set(ResetLayout, Chord::ctrl_shift("L"));

        set(NudgeLeft, Chord::key("Left"));
        set(NudgeRight, Chord::key("Right"));
        set(NudgeUp, Chord::key("Up"));
        set(NudgeDown, Chord::key("Down"));
        set(NudgeAway, Chord::key("PageUp"));
        set(NudgeToward, Chord::key("PageDown"));
        // Align and distribute (issue 70): D for distribute, beside Duplicate's Ctrl+D.
        set(AlignDistribute, Chord::ctrl_shift("D"));
        // Snap to geometry while dragging (issue 77): Ctrl alone, the precise-gesture modifier.
        set(SnapToGeometry, Chord::modifiers(true, false, false));

        let nav = match preset {
            Preset::Default => {
                set(ModeMove, Chord::key("W"));
                set(ModeRotate, Chord::key("E"));
                set(ModeResize, Chord::key("R"));
                set(ModeScale, Chord::key("T"));
                set(MeasureTool, Chord::key("M"));
                NavMap {
                    orbit: Drag::new(MouseButton::Right),
                    // Pan on the plain middle button (issue 72); Shift+right-drag was never found, leaving the view
                    // stuck around its target.
                    pan: Drag::new(MouseButton::Middle),
                    invert_zoom: false,
                }
            }
            Preset::MeshEditor => {
                set(ModeMove, Chord::key("G"));
                set(ModeRotate, Chord::key("R"));
                set(ModeResize, Chord::key("S"));
                set(ModeScale, Chord::shift("S"));
                set(MeasureTool, Chord::key("M"));
                NavMap {
                    orbit: Drag::new(MouseButton::Middle),
                    pan: Drag::with_shift(MouseButton::Middle),
                    invert_zoom: false,
                }
            }
            Preset::Cad => {
                set(ModeMove, Chord::key("M"));
                set(ModeRotate, Chord::key("R"));
                set(ModeResize, Chord::key("T"));
                set(ModeScale, Chord::shift("T"));
                // M is move in this preset, so measure takes K.
                set(MeasureTool, Chord::key("K"));
                NavMap {
                    orbit: Drag::new(MouseButton::Middle),
                    pan: Drag::with_ctrl(MouseButton::Middle),
                    invert_zoom: true,
                }
            }
        };

        let map = Keymap { preset, bindings, nav };
        debug_assert!(map.self_conflicts().is_empty(), "preset {preset:?} ships with a conflict");
        map
    }
}

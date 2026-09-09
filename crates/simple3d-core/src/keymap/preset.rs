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
        // The document keys every tabbed program shares, so nobody has to look
        // them up (issue 61).
        set(CloseTab, Chord::ctrl("W"));
        set(NextTab, Chord::ctrl("Tab"));
        set(PreviousTab, Chord::ctrl_shift("Tab"));
        set(Save, Chord::ctrl("S"));
        set(SaveAs, Chord::ctrl_shift("S"));
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
        // Baking a shape into the triangles it evaluates to (issue 80).
        set(ConvertToMesh, Chord::ctrl_shift("M"));
        // Cutting a shape into a pattern of pieces (issue 82). K for the knife
        // it is, which no preset spends on anything with a Ctrl and a Shift on
        // it.
        set(SplitIntoPieces, Chord::ctrl_shift("K"));
        // The way back from it (issue 82), beside it in every menu and one key
        // along from it: J for "join", which no preset spends on anything else.
        set(Rejoin, Chord::ctrl_shift("J"));
        set(Rename, Chord::key("F2"));
        set(ToggleVisibility, Chord::key("H"));
        set(MoveUp, Chord::ctrl("Up"));
        set(MoveDown, Chord::ctrl("Down"));

        set(FrameSelection, Chord::key("F"));
        set(FrameAll, Chord::shift("F"));
        set(ViewTop, Chord::key("7"));
        set(ViewBottom, Chord::ctrl("7"));
        set(ViewFront, Chord::key("1"));
        set(ViewBack, Chord::ctrl("1"));
        set(ViewRight, Chord::key("3"));
        set(ViewLeft, Chord::ctrl("3"));
        set(ViewIsometric, Chord::key("0"));
        set(ToggleGrid, Chord::key("5"));
        // Alt+X / Y / Z: the axis is named by the key, and none of the three is
        // spoken for by anything else in any preset.
        set(ToggleAxisX, Chord::alt("X"));
        set(ToggleAxisY, Chord::alt("Y"));
        set(ToggleAxisZ, Chord::alt("Z"));
        // The one number left on the row the view switches live on, beside the
        // grid it is a companion to.
        set(ToggleSection, Chord::key("4"));
        set(DisplayShaded, Chord::key("8"));
        set(DisplayShadedEdges, Chord::key("9"));
        set(DisplayWireframe, Chord::key("6"));
        set(ToggleBoundingBox, Chord::key("B"));
        // Tab clears the docks to leave the viewport alone with the model.
        set(ToggleDocks, Chord::key("Tab"));
        set(ResetLayout, Chord::ctrl_shift("L"));

        set(NudgeLeft, Chord::key("Left"));
        set(NudgeRight, Chord::key("Right"));
        set(NudgeUp, Chord::key("Up"));
        set(NudgeDown, Chord::key("Down"));
        set(NudgeAway, Chord::key("PageUp"));
        set(NudgeToward, Chord::key("PageDown"));
        // Held during a drag to snap to geometry. Ctrl on its own (issue 77):
        // it is the modifier a hand already rests on for a precise gesture, it
        // needs no letter key to be free in any preset, and a chord that is
        // modifiers alone can now be bound at all.
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
                    // The middle button on its own, and no modifier (issue 72).
                    // Pan used to be Shift+right-drag: the only navigation that
                    // asked for a modifier, and so the only one nobody found.
                    // What is left when it is not found is orbit and zoom, both
                    // of which turn about the camera's target and leave the
                    // origin exactly where it started -- in the middle of the
                    // frame, which is what "the viewport is locked to the
                    // origin" is. The wheel is beside the button, so a hand
                    // already on the mouse can move about the ground without
                    // reaching for the keyboard at all.
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
                // M names the move tool in this preset, so measure takes K.
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

//! Remappable keyboard and mouse bindings (spec section 8.2).
//!
//! *Every* command and *every* navigation binding is remappable, not a chosen
//! subset -- including the mouse buttons and modifiers for orbit and pan and the
//! wheel direction for zoom. Ships with selectable presets so someone arriving
//! from another program is productive immediately, and a preset is a starting
//! point the user can then modify.
//!
//! Key names are the strings the UI toolkit uses for its own key enum ("A",
//! "Up", "Escape", "F2"), so no translation table can drift out of date. There
//! is a test in the app crate that checks every preset binding against the
//! toolkit's actual key list, so a binding nobody can type cannot ship.
//! A keymap saved on one platform therefore loads sensibly on the other.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// Everything the user can bind. Adding a command here is all it takes for it to
/// appear in the keymap editor, grouped by its area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    New,
    Open,
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
    Rename,
    ToggleVisibility,
    MoveUp,
    MoveDown,

    FrameSelection,
    FrameAll,
    ToggleProjection,
    ViewTop,
    ViewBottom,
    ViewFront,
    ViewBack,
    ViewLeft,
    ViewRight,
    ViewIsometric,
    ToggleGrid,
    DisplayShaded,
    DisplayShadedEdges,
    DisplayWireframe,
    ToggleBoundingBox,
    ToggleGhosts,

    ModeMove,
    ModeRotate,
    ModeResize,
    ToggleHandleFrame,
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

impl Command {
    pub const ALL: &'static [Command] = &[
        Command::New,
        Command::Open,
        Command::Save,
        Command::SaveAs,
        Command::Export,
        Command::Quit,
        Command::Undo,
        Command::Redo,
        Command::Copy,
        Command::Cut,
        Command::Paste,
        Command::Duplicate,
        Command::Delete,
        Command::Group,
        Command::Rename,
        Command::ToggleVisibility,
        Command::MoveUp,
        Command::MoveDown,
        Command::FrameSelection,
        Command::FrameAll,
        Command::ToggleProjection,
        Command::ViewTop,
        Command::ViewBottom,
        Command::ViewFront,
        Command::ViewBack,
        Command::ViewLeft,
        Command::ViewRight,
        Command::ViewIsometric,
        Command::ToggleGrid,
        Command::DisplayShaded,
        Command::DisplayShadedEdges,
        Command::DisplayWireframe,
        Command::ToggleBoundingBox,
        Command::ToggleGhosts,
        Command::ModeMove,
        Command::ModeRotate,
        Command::ModeResize,
        Command::ToggleHandleFrame,
        Command::NudgeLeft,
        Command::NudgeRight,
        Command::NudgeUp,
        Command::NudgeDown,
        Command::NudgeAway,
        Command::NudgeToward,
    ];

    pub fn label(self) -> &'static str {
        use Command::*;
        match self {
            New => "New project",
            Open => "Open project",
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
            Rename => "Rename",
            ToggleVisibility => "Toggle visibility",
            MoveUp => "Move up among siblings",
            MoveDown => "Move down among siblings",
            FrameSelection => "Frame selection",
            FrameAll => "Frame all",
            ToggleProjection => "Perspective / orthographic",
            ViewTop => "View: top",
            ViewBottom => "View: bottom",
            ViewFront => "View: front",
            ViewBack => "View: back",
            ViewLeft => "View: left",
            ViewRight => "View: right",
            ViewIsometric => "View: isometric",
            ToggleGrid => "Show grid",
            DisplayShaded => "Display: shaded",
            DisplayShadedEdges => "Display: shaded with edges",
            DisplayWireframe => "Display: wireframe",
            ToggleBoundingBox => "Show bounding box",
            ToggleGhosts => "Show hidden nodes as ghosts",
            ModeMove => "Manipulator: move",
            ModeRotate => "Manipulator: rotate",
            ModeResize => "Manipulator: resize",
            ToggleHandleFrame => "Handle frame: object / world",
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
            New | Open | Save | SaveAs | Export | Quit => Area::File,
            Undo | Redo | Copy | Cut | Paste | Duplicate | Delete | Group | Rename | ToggleVisibility | MoveUp
            | MoveDown => Area::Edit,
            FrameSelection | FrameAll | ToggleProjection | ViewTop | ViewBottom | ViewFront | ViewBack | ViewLeft
            | ViewRight | ViewIsometric | ToggleGrid | DisplayShaded | DisplayShadedEdges | DisplayWireframe
            | ToggleBoundingBox | ToggleGhosts => Area::View,
            ModeMove | ModeRotate | ModeResize | ToggleHandleFrame | NudgeLeft | NudgeRight | NudgeUp | NudgeDown
            | NudgeAway | NudgeToward => Area::Manipulate,
        }
    }
}

/// A key plus modifiers. Serialised as the text the interface shows -- `Ctrl+S`,
/// `Shift+ArrowUp` -- so an exported keymap is readable and hand-editable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chord {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    pub fn key(key: &str) -> Chord {
        Chord { key: key.to_string(), ctrl: false, shift: false, alt: false }
    }

    pub fn ctrl(key: &str) -> Chord {
        Chord { ctrl: true, ..Chord::key(key) }
    }

    pub fn ctrl_shift(key: &str) -> Chord {
        Chord { ctrl: true, shift: true, ..Chord::key(key) }
    }

    pub fn shift(key: &str) -> Chord {
        Chord { shift: true, ..Chord::key(key) }
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            write!(f, "Ctrl+")?;
        }
        if self.alt {
            write!(f, "Alt+")?;
        }
        if self.shift {
            write!(f, "Shift+")?;
        }
        write!(f, "{}", self.key)
    }
}

impl FromStr for Chord {
    type Err = String;

    fn from_str(text: &str) -> Result<Chord, String> {
        let mut chord = Chord::key("");
        for part in text.split('+') {
            match part.trim() {
                "" => return Err(format!("empty key in binding {text:?}")),
                "Ctrl" | "Control" | "Cmd" | "Command" => chord.ctrl = true,
                "Alt" | "Option" => chord.alt = true,
                "Shift" => chord.shift = true,
                key => chord.key = key.to_string(),
            }
        }
        if chord.key.is_empty() {
            return Err(format!("no key in binding {text:?}"));
        }
        Ok(chord)
    }
}

impl Serialize for Chord {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Chord {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Chord, D::Error> {
        let text = String::deserialize(d)?;
        Chord::from_str(&text).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    pub const ALL: [MouseButton; 3] = [MouseButton::Left, MouseButton::Middle, MouseButton::Right];

    pub fn label(self) -> &'static str {
        match self {
            MouseButton::Left => "Left",
            MouseButton::Middle => "Middle",
            MouseButton::Right => "Right",
        }
    }
}

/// A mouse drag binding for a navigation action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drag {
    pub button: MouseButton,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl Drag {
    pub fn new(button: MouseButton) -> Drag {
        Drag { button, ctrl: false, shift: false, alt: false }
    }

    pub fn with_shift(button: MouseButton) -> Drag {
        Drag { shift: true, ..Drag::new(button) }
    }

    pub fn with_ctrl(button: MouseButton) -> Drag {
        Drag { ctrl: true, ..Drag::new(button) }
    }

    /// Modifier state has to match exactly, so `Shift+Middle` for pan does not
    /// also fire plain-`Middle` orbit.
    pub fn matches(&self, button: MouseButton, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.button == button && self.ctrl == ctrl && self.shift == shift && self.alt == alt
    }
}

impl fmt::Display for Drag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            write!(f, "Ctrl+")?;
        }
        if self.alt {
            write!(f, "Alt+")?;
        }
        if self.shift {
            write!(f, "Shift+")?;
        }
        write!(f, "{} drag", self.button.label())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavMap {
    pub orbit: Drag,
    pub pan: Drag,
    /// Some users, and some other programs, scroll the other way.
    pub invert_zoom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// ScadStudio's own defaults: two buttons and a wheel, so a trackpad or a
    /// two-button mouse is enough.
    Default,
    /// The common mesh-editor convention: middle-drag orbits, G/R/S switch the
    /// manipulator mode.
    MeshEditor,
    /// The common CAD convention: middle-drag orbits, Ctrl+middle pans, and the
    /// wheel zooms the other way.
    Cad,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Default, Preset::MeshEditor, Preset::Cad];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Default => "ScadStudio default",
            Preset::MeshEditor => "Mesh editor",
            Preset::Cad => "CAD",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keymap {
    pub preset: Preset,
    pub bindings: BTreeMap<Command, Chord>,
    pub nav: NavMap,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap::from_preset(Preset::Default)
    }
}

impl Keymap {
    pub fn from_preset(preset: Preset) -> Keymap {
        use Command::*;
        let mut bindings: BTreeMap<Command, Chord> = BTreeMap::new();
        let mut set = |command: Command, chord: Chord| {
            bindings.insert(command, chord);
        };

        set(New, Chord::ctrl("N"));
        set(Open, Chord::ctrl("O"));
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
        set(Rename, Chord::key("F2"));
        set(ToggleVisibility, Chord::key("H"));
        set(MoveUp, Chord::ctrl("Up"));
        set(MoveDown, Chord::ctrl("Down"));

        set(FrameSelection, Chord::key("F"));
        set(FrameAll, Chord::shift("F"));
        set(ToggleProjection, Chord::key("P"));
        set(ViewTop, Chord::key("7"));
        set(ViewBottom, Chord::ctrl("7"));
        set(ViewFront, Chord::key("1"));
        set(ViewBack, Chord::ctrl("1"));
        set(ViewRight, Chord::key("3"));
        set(ViewLeft, Chord::ctrl("3"));
        set(ViewIsometric, Chord::key("0"));
        set(ToggleGrid, Chord::key("5"));
        set(DisplayShaded, Chord::key("8"));
        set(DisplayShadedEdges, Chord::key("9"));
        set(DisplayWireframe, Chord::key("6"));
        set(ToggleBoundingBox, Chord::key("B"));
        set(ToggleGhosts, Chord::shift("H"));

        set(NudgeLeft, Chord::key("Left"));
        set(NudgeRight, Chord::key("Right"));
        set(NudgeUp, Chord::key("Up"));
        set(NudgeDown, Chord::key("Down"));
        set(NudgeAway, Chord::key("PageUp"));
        set(NudgeToward, Chord::key("PageDown"));
        set(ToggleHandleFrame, Chord::key("X"));

        let nav = match preset {
            Preset::Default => {
                set(ModeMove, Chord::key("W"));
                set(ModeRotate, Chord::key("E"));
                set(ModeResize, Chord::key("R"));
                NavMap {
                    orbit: Drag::new(MouseButton::Right),
                    pan: Drag::with_shift(MouseButton::Right),
                    invert_zoom: false,
                }
            }
            Preset::MeshEditor => {
                set(ModeMove, Chord::key("G"));
                set(ModeRotate, Chord::key("R"));
                set(ModeResize, Chord::key("S"));
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

    pub fn binding(&self, command: Command) -> Option<&Chord> {
        self.bindings.get(&command)
    }

    /// The label to show wherever a shortcut appears -- menus, tooltips, help.
    /// Always the *current* binding, never a hardcoded string.
    pub fn shortcut_text(&self, command: Command) -> String {
        self.bindings.get(&command).map(|c| c.to_string()).unwrap_or_default()
    }

    pub fn command_for(&self, chord: &Chord) -> Option<Command> {
        self.bindings.iter().find(|(_, c)| *c == chord).map(|(k, _)| *k)
    }

    /// Which command already holds `chord`, ignoring `command` itself. The
    /// keymap editor names it rather than silently overwriting.
    pub fn conflict(&self, command: Command, chord: &Chord) -> Option<Command> {
        self.bindings.iter().find(|(k, c)| **k != command && *c == chord).map(|(k, _)| *k)
    }

    /// Assign a binding. Refuses and names the holder on a conflict; the caller
    /// then offers to reassign (call again with `force`) or cancel.
    pub fn set(&mut self, command: Command, chord: Chord, force: bool) -> Result<(), Command> {
        if let Some(holder) = self.conflict(command, &chord) {
            if !force {
                return Err(holder);
            }
            self.bindings.remove(&holder);
        }
        self.bindings.insert(command, chord);
        Ok(())
    }

    pub fn unbind(&mut self, command: Command) {
        self.bindings.remove(&command);
    }

    /// Reset one binding to this keymap's preset default.
    pub fn reset(&mut self, command: Command) {
        let preset = Keymap::from_preset(self.preset);
        match preset.bindings.get(&command) {
            Some(chord) => {
                // Clear whoever holds it now, so the reset cannot introduce a
                // conflict of its own.
                if let Some(holder) = self.conflict(command, chord) {
                    self.bindings.remove(&holder);
                }
                self.bindings.insert(command, chord.clone());
            }
            None => {
                self.bindings.remove(&command);
            }
        }
    }

    pub fn reset_all(&mut self) {
        *self = Keymap::from_preset(self.preset);
    }

    pub fn switch_preset(&mut self, preset: Preset) {
        *self = Keymap::from_preset(preset);
    }

    /// Commands sharing a chord. Should always be empty; used by tests and as a
    /// sanity check when importing a hand-edited keymap file.
    pub fn self_conflicts(&self) -> Vec<(Command, Command)> {
        let mut out = Vec::new();
        let entries: Vec<(&Command, &Chord)> = self.bindings.iter().collect();
        for (i, (a, ca)) in entries.iter().enumerate() {
            for (b, cb) in &entries[i + 1..] {
                if ca == cb {
                    out.push((**a, **b));
                }
            }
        }
        out
    }

    pub fn to_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("a keymap always serialises");
        text.push('\n');
        text
    }

    /// Import a keymap file. Anything missing falls back to the recorded
    /// preset's default, so a file from an older build that did not know a
    /// command still yields a fully usable map.
    pub fn from_text(text: &str) -> Result<Keymap, String> {
        let mut map: Keymap = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let preset = Keymap::from_preset(map.preset);
        for command in Command::ALL {
            if !map.bindings.contains_key(command) {
                if let Some(chord) = preset.bindings.get(command) {
                    if map.conflict(*command, chord).is_none() {
                        map.bindings.insert(*command, chord.clone());
                    }
                }
            }
        }
        // Drop commands this build no longer has.
        map.bindings.retain(|k, _| Command::ALL.contains(k));
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_binds_every_command_without_conflicts() {
        for preset in Preset::ALL {
            let map = Keymap::from_preset(preset);
            assert!(map.self_conflicts().is_empty(), "{preset:?}: {:?}", map.self_conflicts());
            for command in Command::ALL {
                assert!(map.binding(*command).is_some(), "{preset:?} does not bind {:?}", command);
            }
        }
    }

    #[test]
    fn every_command_has_a_label_and_an_area() {
        for command in Command::ALL {
            assert!(!command.label().is_empty(), "{command:?}");
            assert!(Area::ALL.contains(&command.area()));
        }
        // The list is complete: every area has at least one command in it.
        for area in Area::ALL {
            assert!(Command::ALL.iter().any(|c| c.area() == area), "{area:?} is empty");
        }
    }

    #[test]
    fn rebinding_onto_a_used_combination_names_the_holder() {
        // Spec section 8.2: "warns, names the command currently holding it, and
        // offers to reassign or cancel. Silently overwriting is not acceptable."
        let mut map = Keymap::default();
        let save = map.binding(Command::Save).unwrap().clone();
        let holder = map.set(Command::Export, save.clone(), false).unwrap_err();
        assert_eq!(holder, Command::Save);
        // Nothing changed while the user decides.
        assert_eq!(map.binding(Command::Save), Some(&save));
        assert_ne!(map.binding(Command::Export), Some(&save));

        // Reassigning takes it away from the previous holder rather than leaving
        // two commands on one chord.
        map.set(Command::Export, save.clone(), true).unwrap();
        assert_eq!(map.binding(Command::Export), Some(&save));
        assert_eq!(map.binding(Command::Save), None);
        assert!(map.self_conflicts().is_empty());
    }

    #[test]
    fn rebinding_a_command_to_its_own_chord_is_not_a_conflict() {
        let mut map = Keymap::default();
        let save = map.binding(Command::Save).unwrap().clone();
        assert!(map.set(Command::Save, save, false).is_ok());
    }

    #[test]
    fn a_single_binding_resets_to_the_preset_default() {
        let mut map = Keymap::default();
        let original = map.binding(Command::Save).unwrap().clone();
        map.set(Command::Save, Chord::ctrl_shift("F9"), true).unwrap();
        assert_ne!(map.binding(Command::Save), Some(&original));
        map.reset(Command::Save);
        assert_eq!(map.binding(Command::Save), Some(&original));
        assert!(map.self_conflicts().is_empty());
    }

    #[test]
    fn resetting_a_binding_takes_it_back_from_whoever_holds_it() {
        let mut map = Keymap::default();
        let save = map.binding(Command::Save).unwrap().clone();
        map.set(Command::Export, save.clone(), true).unwrap();
        map.reset(Command::Save);
        assert_eq!(map.binding(Command::Save), Some(&save));
        assert!(map.self_conflicts().is_empty());
    }

    #[test]
    fn the_whole_map_resets_to_the_preset() {
        let mut map = Keymap::from_preset(Preset::MeshEditor);
        let pristine = map.clone();
        map.set(Command::Save, Chord::key("F9"), true).unwrap();
        map.set(Command::ModeMove, Chord::key("F10"), true).unwrap();
        map.reset_all();
        assert_eq!(map, pristine);
    }

    #[test]
    fn switching_preset_changes_navigation_and_mode_keys() {
        // Spec section 8.2, acceptance criterion 27.
        let default = Keymap::from_preset(Preset::Default);
        let mesh = Keymap::from_preset(Preset::MeshEditor);
        let cad = Keymap::from_preset(Preset::Cad);
        assert_ne!(default.nav.orbit, mesh.nav.orbit);
        assert_ne!(mesh.nav.pan, cad.nav.pan);
        assert!(cad.nav.invert_zoom);
        assert_eq!(mesh.binding(Command::ModeMove), Some(&Chord::key("G")));
        assert_eq!(default.binding(Command::ModeMove), Some(&Chord::key("W")));
    }

    #[test]
    fn a_keymap_round_trips_through_its_file_form() {
        // Spec section 8.2: export and import as one file, to carry between machines.
        let mut map = Keymap::from_preset(Preset::Cad);
        map.set(Command::Export, Chord::ctrl_shift("E"), true).unwrap();
        map.nav.orbit = Drag::with_ctrl(MouseButton::Left);
        map.nav.invert_zoom = false;

        let text = map.to_text();
        assert!(text.contains("\"export\": \"Ctrl+Shift+E\""), "{text}");
        let back = Keymap::from_text(&text).unwrap();
        assert_eq!(back, map);
    }

    #[test]
    fn chords_round_trip_through_their_display_form() {
        for chord in [
            Chord::key("A"),
            Chord::ctrl("S"),
            Chord::ctrl_shift("Z"),
            Chord::shift("Up"),
            Chord { key: "F5".into(), ctrl: true, shift: true, alt: true },
        ] {
            assert_eq!(Chord::from_str(&chord.to_string()).unwrap(), chord);
        }
        assert_eq!(Chord::from_str("Cmd+S").unwrap(), Chord::ctrl("S"));
        assert!(Chord::from_str("").is_err());
        assert!(Chord::from_str("Ctrl+").is_err());
    }

    #[test]
    fn an_imported_map_missing_a_command_falls_back_to_its_preset() {
        let mut map = Keymap::default();
        map.unbind(Command::FrameAll);
        let text = map.to_text();
        let back = Keymap::from_text(&text).unwrap();
        assert_eq!(back.binding(Command::FrameAll), Keymap::default().binding(Command::FrameAll));
        assert!(back.self_conflicts().is_empty());
    }

    #[test]
    fn an_imported_map_with_an_unknown_command_is_not_broken_by_it() {
        let text = r#"{
          "preset": "default",
          "bindings": { "save": "Ctrl+S", "teleport": "Ctrl+T" },
          "nav": { "orbit": { "button": "right" }, "pan": { "button": "right", "shift": true },
                   "invert_zoom": false }
        }"#;
        // An unknown command name is a hard error from serde, which is the safe
        // outcome for a file this build cannot fully honour.
        assert!(Keymap::from_text(text).is_err());
    }

    #[test]
    fn drag_modifiers_must_match_exactly() {
        let pan = Drag::with_shift(MouseButton::Right);
        assert!(pan.matches(MouseButton::Right, false, true, false));
        assert!(!pan.matches(MouseButton::Right, false, false, false));
        assert!(!pan.matches(MouseButton::Right, true, true, false));
        let orbit = Drag::new(MouseButton::Right);
        assert!(!orbit.matches(MouseButton::Right, false, true, false), "orbit fired on the pan chord");
    }

    #[test]
    fn shortcut_text_is_what_the_menus_show() {
        let map = Keymap::default();
        assert_eq!(map.shortcut_text(Command::Save), "Ctrl+S");
        assert_eq!(map.shortcut_text(Command::SaveAs), "Ctrl+Shift+S");
        assert_eq!(map.shortcut_text(Command::Delete), "Delete");
    }

    #[test]
    fn a_chord_resolves_back_to_its_command() {
        let map = Keymap::default();
        assert_eq!(map.command_for(&Chord::ctrl("S")), Some(Command::Save));
        assert_eq!(map.command_for(&Chord::ctrl_shift("S")), Some(Command::SaveAs));
        assert_eq!(map.command_for(&Chord::key("Backslash")), None);
    }
}

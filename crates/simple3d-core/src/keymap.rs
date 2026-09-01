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
    Rename,
    ToggleVisibility,
    MoveUp,
    MoveDown,

    FrameSelection,
    FrameAll,
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
    ToggleHandleFrame,
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

impl Command {
    pub const ALL: &'static [Command] = &[
        Command::New,
        Command::Open,
        Command::CloseTab,
        Command::NextTab,
        Command::PreviousTab,
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
        Command::Pattern,
        Command::Rename,
        Command::ToggleVisibility,
        Command::MoveUp,
        Command::MoveDown,
        Command::FrameSelection,
        Command::FrameAll,
        Command::ViewTop,
        Command::ViewBottom,
        Command::ViewFront,
        Command::ViewBack,
        Command::ViewLeft,
        Command::ViewRight,
        Command::ViewIsometric,
        Command::ToggleGrid,
        Command::ToggleAxisX,
        Command::ToggleAxisY,
        Command::ToggleAxisZ,
        Command::DisplayShaded,
        Command::DisplayShadedEdges,
        Command::DisplayWireframe,
        Command::ToggleBoundingBox,
        Command::ToggleDocks,
        Command::ResetLayout,
        Command::ModeMove,
        Command::ModeRotate,
        Command::ModeResize,
        Command::ModeScale,
        Command::ToggleHandleFrame,
        Command::MeasureTool,
        Command::SnapToGeometry,
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
            Undo | Redo | Copy | Cut | Paste | Duplicate | Delete | Group | Pattern | Rename | ToggleVisibility
            | MoveUp | MoveDown => Area::Edit,
            FrameSelection | FrameAll | ViewTop | ViewBottom | ViewFront | ViewBack | ViewLeft | ViewRight
            | ViewIsometric | ToggleGrid | ToggleAxisX | ToggleAxisY | ToggleAxisZ | DisplayShaded
            | DisplayShadedEdges | DisplayWireframe | ToggleBoundingBox | ToggleDocks | ResetLayout => Area::View,
            ModeMove | ModeRotate | ModeResize | ModeScale | ToggleHandleFrame | MeasureTool | SnapToGeometry
            | NudgeLeft | NudgeRight | NudgeUp | NudgeDown | NudgeAway | NudgeToward => Area::Manipulate,
        }
    }
}

/// A key plus modifiers. Serialised as the text the interface shows -- `Ctrl+S`,
/// `Shift+ArrowUp` -- so an exported keymap is readable and hand-editable.
///
/// `keys` may hold any number of them, including none.
///
/// None at all makes the chord the modifiers themselves: `Ctrl` on its own is a
/// binding (issue 77). A modifier is a key like any other -- the one thing a
/// hold-to-snap binding actually wants -- so refusing to store one only meant
/// nobody could bind what they were already reaching for.
///
/// Several makes it a combination of ordinary keys: `Q+W+E` is a binding
/// too, and no key is only ever a *base* for one. What makes both work is the
/// same rule, and it is the one a hand already performs: whatever is held down
/// together is the chord, and it is complete when the hand comes off it.
///
/// The keys are kept sorted, so a chord is the *set* that was held and `Q+W`
/// cannot be bound separately from `W+Q` -- one press cannot mean two things.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chord {
    pub keys: Vec<String>,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    pub fn key(key: &str) -> Chord {
        Chord { keys: vec![key.to_string()], ctrl: false, shift: false, alt: false }
    }

    /// A combination of ordinary keys, in any order: the set is what counts.
    pub fn combo<I: IntoIterator<Item = S>, S: AsRef<str>>(keys: I) -> Chord {
        let mut chord = Chord {
            keys: keys.into_iter().map(|k| k.as_ref().to_string()).collect(),
            ..Chord::modifiers(false, false, false)
        };
        chord.normalise();
        chord
    }

    /// A chord that is nothing but modifiers, such as the default hold for
    /// geometry snapping.
    pub fn modifiers(ctrl: bool, shift: bool, alt: bool) -> Chord {
        Chord { keys: Vec::new(), ctrl, shift, alt }
    }

    /// Whether this chord is modifiers alone. Such a chord can never be produced
    /// by a key event, so everything that resolves a key press has to skip it,
    /// and everything that reads a held state has to accept it.
    pub fn is_modifier_only(&self) -> bool {
        self.keys.is_empty()
    }

    /// Sorted and deduplicated, which is what makes a chord a set.
    fn normalise(&mut self) {
        self.keys.sort();
        self.keys.dedup();
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

    pub fn alt(key: &str) -> Chord {
        Chord { alt: true, ..Chord::key(key) }
    }

    /// Whether every key of this chord is in `held`, and its modifiers are
    /// exactly the ones down. What both a hold ("is the snap key down?") and a
    /// press ("did this complete a combination?") ask.
    pub fn satisfied_by(&self, held: impl Fn(&str) -> bool, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.ctrl == ctrl && self.shift == shift && self.alt == alt && self.keys.iter().all(|k| held(k))
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Joined rather than each modifier written with a trailing `+`, so a
        // modifier-only chord reads as `Ctrl`, not as `Ctrl+` with nothing after
        // it -- and still round-trips through `from_str`.
        let mut parts: Vec<&str> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.extend(self.keys.iter().map(String::as_str));
        write!(f, "{}", parts.join("+"))
    }
}

impl FromStr for Chord {
    type Err = String;

    fn from_str(text: &str) -> Result<Chord, String> {
        let mut chord = Chord::modifiers(false, false, false);
        for part in text.split('+') {
            match part.trim() {
                "" => return Err(format!("empty key in binding {text:?}")),
                "Ctrl" | "Control" | "Cmd" | "Command" => chord.ctrl = true,
                "Alt" | "Option" => chord.alt = true,
                "Shift" => chord.shift = true,
                key => chord.keys.push(key.to_string()),
            }
        }
        // Modifiers alone are a binding of their own (issue 77); a chord with
        // neither a key nor a modifier is not.
        if chord.keys.is_empty() && !(chord.ctrl || chord.shift || chord.alt) {
            return Err(format!("no key in binding {text:?}"));
        }
        chord.normalise();
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
    /// Simple 3D's own defaults: two buttons and a wheel, so a trackpad or a
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
            Preset::Default => "Simple 3D default",
            Preset::MeshEditor => "Mesh editor",
            Preset::Cad => "CAD",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keymap {
    pub preset: Preset,
    #[serde(deserialize_with = "bindings_from_names")]
    pub bindings: BTreeMap<Command, Chord>,
    pub nav: NavMap,
}

/// Command names earlier builds wrote and this one no longer has.
///
/// A keymap naming one still loads: that binding is dropped and the rest of the
/// user's map survives, because a command being retired is our doing and must
/// not cost them the map they carry between machines. A name that was never a
/// command of ours stays a hard error -- that file is not one this build can
/// honour, and quietly loading half of it is the worse answer.
const RETIRED: [&str; 2] = ["toggle_projection", "toggle_ghosts"];

/// Bindings an older build wrote as *its* default, which this build has moved.
///
/// A keymap file is written back on any change, so every map saved by an older
/// build carries that build's whole default set whether or not the user ever
/// chose any of it. Left alone, a changed default would reach only people with
/// no keymap file -- which is nobody who has used the program. So a binding that
/// is still exactly the old default is moved to the new one, and a binding the
/// user has since changed is left alone, because that one they did choose.
///
/// Snap-to-geometry moved from V to Ctrl on its own when a modifier became
/// bindable at all (issue 77).
const MOVED_DEFAULTS: [(Command, &str, &str); 1] = [(Command::SnapToGeometry, "V", "Ctrl")];

fn bindings_from_names<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<Command, Chord>, D::Error> {
    use serde::de::IntoDeserializer;
    let named: BTreeMap<String, Chord> = BTreeMap::deserialize(deserializer)?;
    let mut bindings = BTreeMap::new();
    for (name, chord) in named {
        if RETIRED.contains(&name.as_str()) {
            continue;
        }
        let command: Command =
            Command::deserialize(IntoDeserializer::<serde::de::value::Error>::into_deserializer(name.as_str()))
                .map_err(|_| D::Error::custom(format!("unknown command \"{name}\"")))?;
        bindings.insert(command, chord);
    }
    Ok(bindings)
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
        set(ToggleHandleFrame, Chord::key("X"));
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
                    pan: Drag::with_shift(MouseButton::Right),
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

    /// What a key press fires, given everything held down at that moment.
    ///
    /// The longest chord fully satisfied wins, so with Q and W already down,
    /// pressing E fires `Q+W+E` rather than whatever `E` alone is bound to. Only
    /// a chord containing the key just pressed is considered: a combination
    /// fires on the press that completes it, not again on every key afterwards.
    ///
    /// A key that is *part* of a longer combination still fires its own binding
    /// on its own press -- nothing can know a longer one is coming -- so a
    /// combination is worth building out of keys that are otherwise free.
    pub fn command_for_press(
        &self,
        pressed: &str,
        held: impl Fn(&str) -> bool,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> Option<Command> {
        self.bindings
            .iter()
            .filter(|(_, chord)| chord.keys.iter().any(|k| k == pressed))
            .filter(|(_, chord)| chord.satisfied_by(&held, ctrl, shift, alt))
            .max_by_key(|(_, chord)| chord.keys.len())
            .map(|(command, _)| *command)
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
        for (command, was, now) in MOVED_DEFAULTS {
            let (was, now) = (Chord::from_str(was)?, Chord::from_str(now)?);
            if map.bindings.get(&command) == Some(&was) && map.conflict(command, &now).is_none() {
                map.bindings.insert(command, now);
            }
        }
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

    /// Spec acceptance criterion 27 as one sequence: switch preset, *then* rebind
    /// onto a combination already in use, and the conflict is named.
    ///
    /// The two halves are covered separately above, but the criterion asks for
    /// them in order, and a preset switch is exactly what could leave the map in
    /// a state where the conflict check looks at the wrong bindings.
    #[test]
    fn a_conflict_is_named_after_switching_preset_too() {
        let mut map = Keymap::default();
        map.switch_preset(Preset::MeshEditor);
        assert_eq!(map.preset, Preset::MeshEditor);
        assert!(map.self_conflicts().is_empty(), "the switch itself introduced a conflict");

        // A chord this preset genuinely holds -- not one carried over from the
        // preset we came from.
        let (holder, chord) = (Command::ModeMove, map.binding(Command::ModeMove).unwrap().clone());
        assert_eq!(chord, Chord::key("G"), "MeshEditor's ModeMove binding changed; pick another chord");

        let named = map.set(Command::Rename, chord.clone(), false).unwrap_err();
        assert_eq!(named, holder, "the conflict named the wrong command");
        // Refused, not silently overwritten: both bindings are as they were.
        assert_eq!(map.binding(Command::ModeMove), Some(&chord));
        assert_ne!(map.binding(Command::Rename), Some(&chord));

        // Reassigning on confirmation takes it from the holder rather than
        // leaving two commands on one chord.
        map.set(Command::Rename, chord.clone(), true).unwrap();
        assert_eq!(map.binding(Command::Rename), Some(&chord));
        assert_eq!(map.binding(Command::ModeMove), None);
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
            Chord { keys: vec!["F5".into()], ctrl: true, shift: true, alt: true },
            Chord::combo(["Q", "W", "E"]),
        ] {
            assert_eq!(Chord::from_str(&chord.to_string()).unwrap(), chord);
        }
        assert_eq!(Chord::from_str("Cmd+S").unwrap(), Chord::ctrl("S"));
        assert!(Chord::from_str("").is_err());
        assert!(Chord::from_str("Ctrl+").is_err());
    }

    #[test]
    fn a_default_that_has_moved_is_carried_over_but_a_chosen_binding_is_not() {
        // Issue 77: snap-to-geometry moved from V to Ctrl. Every keymap file
        // written before that carries "V" -- as the old default, not as anyone's
        // choice -- so loading one has to move it, or the new default reaches
        // only a user who has never saved a keymap.
        let stored = r#"{
          "preset": "default",
          "bindings": { "snap_to_geometry": "V" },
          "nav": { "orbit": { "button": "right" }, "pan": { "button": "right", "shift": true },
                   "invert_zoom": false }
        }"#;
        let map = Keymap::from_text(stored).unwrap();
        assert_eq!(map.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));

        // A binding the user has since changed is theirs, and is left alone.
        let chosen = stored.replace("\"V\"", "\"Shift+K\"");
        let map = Keymap::from_text(&chosen).unwrap();
        assert_eq!(map.binding(Command::SnapToGeometry), Some(&Chord::shift("K")));

        // And the move never takes a chord out from under another command.
        let taken = stored
            .replace("\"snap_to_geometry\": \"V\"", "\"snap_to_geometry\": \"V\", \"toggle_bounding_box\": \"Ctrl\"");
        let map = Keymap::from_text(&taken).unwrap();
        assert_eq!(map.binding(Command::SnapToGeometry), Some(&Chord::key("V")));
        assert_eq!(map.binding(Command::ToggleBoundingBox), Some(&Chord::modifiers(true, false, false)));
        assert!(map.self_conflicts().is_empty());
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
    fn a_modifier_on_its_own_is_a_chord() {
        // Issue 77: Ctrl, Shift and Alt used to be unbindable because a chord
        // was required to carry a key as well.
        let ctrl = Chord::modifiers(true, false, false);
        assert!(ctrl.is_modifier_only());
        assert_eq!(ctrl.to_string(), "Ctrl");
        assert_eq!(Chord::from_str("Ctrl").unwrap(), ctrl);

        let all = Chord::modifiers(true, true, true);
        assert_eq!(all.to_string(), "Ctrl+Alt+Shift");
        assert_eq!(Chord::from_str(&all.to_string()).unwrap(), all);

        // A key chord is still not modifier-only, and nothing at all is still an
        // error.
        assert!(!Chord::ctrl("S").is_modifier_only());
        assert!(Chord::from_str("").is_err());
        assert!(Chord::from_str("Ctrl+").is_err());
    }

    #[test]
    fn a_modifier_only_chord_is_a_binding_of_its_own_not_a_prefix() {
        // Ctrl and Ctrl+S are different bindings: holding Ctrl to snap must not
        // collide with Save, in either direction.
        let map = Keymap::default();
        assert_eq!(map.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));
        assert_eq!(map.command_for(&Chord::ctrl("S")), Some(Command::Save));
        assert_eq!(map.command_for(&Chord::modifiers(true, false, false)), Some(Command::SnapToGeometry));
        assert!(map.self_conflicts().is_empty());
        assert_eq!(map.shortcut_text(Command::SnapToGeometry), "Ctrl");
    }

    #[test]
    fn a_keymap_holding_a_modifier_only_binding_round_trips() {
        let mut map = Keymap::default();
        map.set(Command::ToggleBoundingBox, Chord::modifiers(false, true, true), true).unwrap();
        let text = map.to_text();
        assert!(text.contains("\"snap_to_geometry\": \"Ctrl\""), "{text}");
        assert!(text.contains("\"toggle_bounding_box\": \"Alt+Shift\""), "{text}");
        assert_eq!(Keymap::from_text(&text).unwrap(), map);
    }

    #[test]
    fn a_combination_of_ordinary_keys_is_a_chord_too() {
        // Any key can be the base of a combination, not only Ctrl, Shift and
        // Alt: Q+W+E is a binding.
        let mut map = Keymap::default();
        let combo = Chord::combo(["Q", "W", "E"]);
        assert_eq!(combo.to_string(), "E+Q+W", "the keys are a set, written in one settled order");
        assert_eq!(Chord::from_str("Q+W+E").unwrap(), combo, "the order they are written in must not matter");
        map.set(Command::FrameAll, combo.clone(), true).unwrap();

        let down = |name: &str| ["Q", "W", "E"].contains(&name);
        assert_eq!(map.command_for_press("E", down, false, false, false), Some(Command::FrameAll));
        // The press that completes it is the one that fires. Q and W are in the
        // chord too, but pressing them again with everything already down is not
        // a second completion of a different binding.
        assert_eq!(map.command_for_press("Q", down, false, false, false), Some(Command::FrameAll));
        // A key of the combination pressed on its own does not fire it -- it
        // fires whatever that key is bound to by itself, which here is the
        // preset's own rotate.
        assert_eq!(map.command_for_press("E", |name| name == "E", false, false, false), Some(Command::ModeRotate));
        // Nor does the combination fire with a modifier held that it does not
        // carry: Ctrl+E is Export, and stays Export with Q and W down.
        assert_eq!(map.command_for_press("E", down, true, false, false), Some(Command::Export));
    }

    #[test]
    fn the_longest_binding_the_held_keys_satisfy_is_the_one_that_fires() {
        // With Q and W down, pressing E fires Q+W+E rather than whatever E alone
        // is bound to -- otherwise a combination could never be built out of
        // keys that are already in use.
        let mut map = Keymap::default();
        map.set(Command::FrameSelection, Chord::key("E"), true).unwrap();
        map.set(Command::FrameAll, Chord::combo(["Q", "W", "E"]), true).unwrap();

        let all_down = |name: &str| ["Q", "W", "E"].contains(&name);
        assert_eq!(map.command_for_press("E", all_down, false, false, false), Some(Command::FrameAll));
        assert_eq!(map.command_for_press("E", |name| name == "E", false, false, false), Some(Command::FrameSelection));
    }

    #[test]
    fn a_chord_resolves_back_to_its_command() {
        let map = Keymap::default();
        assert_eq!(map.command_for(&Chord::ctrl("S")), Some(Command::Save));
        assert_eq!(map.command_for(&Chord::ctrl_shift("S")), Some(Command::SaveAs));
        assert_eq!(map.command_for(&Chord::key("Backslash")), None);
    }
}

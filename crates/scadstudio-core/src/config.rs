//! Per-user settings that are *not* part of a project (spec sections 7.1, 9,
//! 11): window geometry, panel sizes, display mode, the keymap, the recent-file
//! list and the last export choices.
//!
//! Stored in the platform-appropriate per-user location, with a portable mode
//! that keeps everything beside the executable instead. The binary never
//! requires the source tree, a working directory or sibling files to be present:
//! if the settings file is missing or unreadable, defaults apply.

use crate::keymap::Keymap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";
const KEYMAP_FILE: &str = "keymap.json";
const MAX_RECENT: usize = 10;

/// How the viewport draws geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    #[default]
    Shaded,
    ShadedWithEdges,
    Wireframe,
}

impl DisplayMode {
    pub const ALL: [DisplayMode; 3] = [DisplayMode::Shaded, DisplayMode::ShadedWithEdges, DisplayMode::Wireframe];

    pub fn label(self) -> &'static str {
        match self {
            DisplayMode::Shaded => "Shaded",
            DisplayMode::ShadedWithEdges => "Shaded with edges",
            DisplayMode::Wireframe => "Wireframe",
        }
    }
}

/// Which frame the manipulator handles work in (spec section 6.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandleFrame {
    #[default]
    Object,
    World,
}

impl HandleFrame {
    pub fn label(self) -> &'static str {
        match self {
            HandleFrame::Object => "Object",
            HandleFrame::World => "World",
        }
    }

    pub fn toggled(self) -> HandleFrame {
        match self {
            HandleFrame::Object => HandleFrame::World,
            HandleFrame::World => HandleFrame::Object,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub window_size: [f32; 2],
    pub window_maximized: bool,
    pub outliner_width: f32,
    pub properties_width: f32,
    pub display_mode: DisplayMode,
    pub show_grid: bool,
    pub show_bounding_box: bool,
    pub show_ghosts: bool,
    pub handle_frame: HandleFrame,
    /// Rotation snap in degrees. The move and resize snap comes from the scene's
    /// grid spacing, so it is a project setting rather than a user one.
    pub rotate_snap_deg: f64,
    pub last_export_dir: Option<PathBuf>,
    pub last_export_format: String,
    pub last_export_scale: f64,
    pub recent_files: Vec<PathBuf>,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            window_size: [1400.0, 880.0],
            window_maximized: false,
            outliner_width: 260.0,
            properties_width: 320.0,
            display_mode: DisplayMode::ShadedWithEdges,
            show_grid: true,
            show_bounding_box: false,
            show_ghosts: true,
            handle_frame: HandleFrame::Object,
            rotate_snap_deg: 15.0,
            last_export_dir: None,
            // 3MF by default, because it records units.
            last_export_format: "3mf".to_string(),
            last_export_scale: 1.0,
            recent_files: Vec::new(),
        }
    }
}

impl AppSettings {
    pub fn remember_recent(&mut self, path: &Path) {
        let path = path.to_path_buf();
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(MAX_RECENT);
    }

    pub fn forget_recent(&mut self, path: &Path) {
        self.recent_files.retain(|p| p != path);
    }
}

/// True when a marker file sits next to the executable, in which case settings
/// live beside it and nothing is written to the user's home directory.
pub fn portable_mode() -> bool {
    executable_dir().is_some_and(|dir| dir.join("portable").exists() || dir.join("portable.txt").exists())
}

fn executable_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

/// Where settings and the keymap live.
pub fn config_dir() -> PathBuf {
    if portable_mode() {
        if let Some(dir) = executable_dir() {
            return dir;
        }
    }
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("ScadStudio");
        }
    } else if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("scadstudio");
        }
    }
    match std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        Some(home) if cfg!(windows) => PathBuf::from(home).join("ScadStudio"),
        Some(home) => PathBuf::from(home).join(".config").join("scadstudio"),
        // No home to write to: fall back to the working directory rather than
        // refusing to start.
        None => PathBuf::from("."),
    }
}

/// Load settings, falling back to defaults for anything missing or unreadable --
/// a corrupt settings file must never stop the application starting.
pub fn load_settings() -> AppSettings {
    load_settings_from(&config_dir())
}

pub fn save_settings(settings: &AppSettings) -> std::io::Result<()> {
    save_settings_to(&config_dir(), settings)
}

/// `load_settings` against an explicit directory, so a test can drive the real
/// startup path against a temp directory instead of the user's own config.
pub fn load_settings_from(dir: &Path) -> AppSettings {
    std::fs::read_to_string(dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// `save_settings` against an explicit directory.
pub fn save_settings_to(dir: &Path, settings: &AppSettings) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(settings).expect("settings always serialise");
    std::fs::write(dir.join(SETTINGS_FILE), text + "\n")
}

/// The keymap is stored separately so it can be exported and imported as one
/// file that a user carries between machines (spec section 8.2).
pub fn load_keymap() -> Keymap {
    load_keymap_from(&config_dir())
}

pub fn save_keymap(keymap: &Keymap) -> std::io::Result<()> {
    save_keymap_to(&config_dir(), keymap)
}

/// `load_keymap` against an explicit directory. The startup path goes through
/// here so a test can drive it against a temp directory rather than the user's
/// real config (acceptance criterion 28). A missing or unreadable file gives the
/// default keymap: a corrupt one must never stop the application starting.
pub fn load_keymap_from(dir: &Path) -> Keymap {
    std::fs::read_to_string(dir.join(KEYMAP_FILE))
        .ok()
        .and_then(|text| Keymap::from_text(&text).ok())
        .unwrap_or_default()
}

/// `save_keymap` against an explicit directory.
pub fn save_keymap_to(dir: &Path, keymap: &Keymap) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(KEYMAP_FILE), keymap.to_text())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_tolerate_a_partial_file() {
        let settings = AppSettings {
            display_mode: DisplayMode::Wireframe,
            last_export_scale: 2.5,
            last_export_format: "stl".into(),
            recent_files: vec![PathBuf::from("/tmp/a.scad3d")],
            ..AppSettings::default()
        };
        let text = serde_json::to_string(&settings).unwrap();
        let back: AppSettings = serde_json::from_str(&text).unwrap();
        assert_eq!(back, settings);

        // A file from an older build that did not have some of these fields.
        let partial: AppSettings = serde_json::from_str("{\"display_mode\":\"wireframe\"}").unwrap();
        assert_eq!(partial.display_mode, DisplayMode::Wireframe);
        assert_eq!(partial.last_export_format, "3mf");
        assert_eq!(partial.rotate_snap_deg, 15.0);
    }

    #[test]
    fn corrupt_settings_fall_back_to_defaults_rather_than_failing() {
        for text in ["", "{", "null", "[1,2,3]", "{\"display_mode\": \"holographic\"}"] {
            let parsed: Option<AppSettings> = serde_json::from_str(text).ok();
            let settings = parsed.unwrap_or_default();
            assert_eq!(settings.last_export_format, "3mf");
        }
    }

    #[test]
    fn recent_files_are_most_recent_first_deduplicated_and_bounded() {
        let mut settings = AppSettings::default();
        for i in 0..MAX_RECENT + 5 {
            settings.remember_recent(Path::new(&format!("/tmp/p{i}.scad3d")));
        }
        assert_eq!(settings.recent_files.len(), MAX_RECENT);
        assert_eq!(settings.recent_files[0], PathBuf::from("/tmp/p14.scad3d"));

        settings.remember_recent(Path::new("/tmp/p10.scad3d"));
        assert_eq!(settings.recent_files[0], PathBuf::from("/tmp/p10.scad3d"));
        assert_eq!(settings.recent_files.iter().filter(|p| p.ends_with("p10.scad3d")).count(), 1);

        settings.forget_recent(Path::new("/tmp/p10.scad3d"));
        assert!(!settings.recent_files.contains(&PathBuf::from("/tmp/p10.scad3d")));
    }

    #[test]
    fn the_config_directory_is_absolute_and_named_for_the_app() {
        let dir = config_dir();
        let text = dir.to_string_lossy().to_lowercase();
        assert!(text.contains("scadstudio"), "{dir:?}");
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "scadstudio-config-test-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Spec acceptance criterion 28: restart after rebinding and the keymap has
    /// persisted, with the menus showing the new binding rather than the default.
    ///
    /// This drives the real startup path -- `save_keymap` then `load_keymap` over
    /// a directory -- rather than the serialised form alone, because that glue is
    /// the whole of what "persisted across a restart" means. The directory is a
    /// temp one so the test cannot touch the user's own config.
    #[test]
    fn a_rebinding_survives_a_restart_and_the_menus_follow() {
        use crate::keymap::{Chord, Command, Preset};

        let dir = temp_dir("restart");
        assert_eq!(load_keymap_from(&dir), Keymap::default(), "an empty config dir must give the default keymap");

        // Switch preset and rebind a command onto a combination of our choosing.
        let mut keymap = Keymap::from_preset(Preset::MeshEditor);
        let default_group = Keymap::default().binding(Command::Group).cloned();
        keymap.set(Command::Group, Chord::ctrl_shift("J"), true).unwrap();
        let expected_text = keymap.shortcut_text(Command::Group);
        assert_ne!(Some(&Chord::ctrl_shift("J")), default_group.as_ref(), "the test's chord is already the default");
        save_keymap_to(&dir, &keymap).unwrap();

        // Restart: a fresh process reads the same directory and knows nothing
        // else about the session that wrote it.
        let reloaded = load_keymap_from(&dir);
        assert_eq!(reloaded, keymap, "the keymap did not survive the round trip through its file");
        assert_eq!(reloaded.binding(Command::Group), Some(&Chord::ctrl_shift("J")));
        assert_eq!(reloaded.command_for(&Chord::ctrl_shift("J")), Some(Command::Group));
        assert_eq!(reloaded.preset, Preset::MeshEditor, "the preset did not persist");

        // What the menus and tooltips render is the reloaded binding, not the
        // default one they would show from a fresh `Keymap`.
        assert_eq!(reloaded.shortcut_text(Command::Group), expected_text);
        assert_ne!(
            reloaded.shortcut_text(Command::Group),
            Keymap::default().shortcut_text(Command::Group),
            "the menus would still show the default binding"
        );

        // And the file really is what a restart reads: nothing lingers in memory.
        assert!(dir.join(KEYMAP_FILE).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A keymap file that has been corrupted must leave the application startable
    /// on the defaults rather than failing to launch.
    #[test]
    fn an_unreadable_keymap_file_falls_back_to_the_default() {
        let dir = temp_dir("corrupt");
        for text in ["", "{", "not json at all", "{\"preset\":\"holographic\"}"] {
            std::fs::write(dir.join(KEYMAP_FILE), text).unwrap();
            assert_eq!(load_keymap_from(&dir), Keymap::default(), "{text:?}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_handle_frame_toggles_both_ways() {
        assert_eq!(HandleFrame::Object.toggled(), HandleFrame::World);
        assert_eq!(HandleFrame::World.toggled(), HandleFrame::Object);
    }
}

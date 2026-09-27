//! Where settings live: beside a portable executable, else the user profile.

use std::path::PathBuf;

/// True when a marker file sits by the executable; settings then live there and nothing goes to home.
pub fn portable_mode() -> bool {
    executable_dir().is_some_and(|dir| dir.join("portable").exists() || dir.join("portable.txt").exists())
}

pub(crate) fn executable_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

pub fn config_dir() -> PathBuf {
    if portable_mode() {
        if let Some(dir) = executable_dir() {
            return dir;
        }
    }
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("Simple3D");
        }
    } else if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("simple3d");
        }
    }
    match std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        Some(home) if cfg!(windows) => PathBuf::from(home).join("Simple3D"),
        Some(home) => PathBuf::from(home).join(".config").join("simple3d"),
        // No home: fall back to the working directory rather than refusing to start.
        None => PathBuf::from("."),
    }
}

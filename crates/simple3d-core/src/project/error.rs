//! What can stop a project file being read.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// A file written by a newer version. Refused with a clear message rather
    /// than half-understood.
    TooNew { found: u32, supported: u32 },
    /// Not valid JSON at all -- truncated mid-file, or not a project file. The
    /// message names the position and what was expected.
    Malformed(String),
    /// Valid JSON, but not a valid scene: an unknown primitive type, a missing
    /// required field.
    Invalid(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::TooNew { found, supported } => write!(
                f,
                "This project was saved in format version {found}, but this build only understands \
                 version {supported}. Use a newer version of Simple 3D to open it."
            ),
            LoadError::Malformed(why) => write!(f, "The project file could not be read: {why}"),
            LoadError::Invalid(why) => write!(f, "The project file is not a valid scene: {why}"),
        }
    }
}

impl std::error::Error for LoadError {}

pub(crate) fn describe(e: &serde_json::Error) -> String {
    format!("{e}")
}

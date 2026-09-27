//! What can stop a project file being read.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// Written by a newer version; refused clearly rather than half-understood.
    TooNew { found: u32, supported: u32 },
    /// Not valid JSON (truncated or not a project); the message names the position and what was expected.
    Malformed(String),
    /// Valid JSON but not a valid scene, such as an unknown type or missing field.
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

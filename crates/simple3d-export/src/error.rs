//! What can stop an export, in the words the dialog shows.

use std::fmt;

/// Why an export did not happen. Every variant carries the specific reason, so
/// the dialog never has to show a generic message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    Empty,
    /// Verification found problems. Each string names what is wrong; the caller
    /// prefixes the node responsible.
    Invalid(Vec<String>),
    Cancelled,
    Io(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Empty => write!(f, "There is nothing to export: the scene has no visible geometry."),
            ExportError::Invalid(problems) => {
                writeln!(f, "The mesh is not valid for 3D printing, so nothing was written:")?;
                for problem in problems {
                    writeln!(f, "  - {problem}")?;
                }
                write!(f, "Fix the reported nodes, or export anyway if you know what you are doing.")
            }
            ExportError::Cancelled => write!(f, "Export cancelled. No file was written."),
            ExportError::Io(why) => write!(f, "Writing the file failed: {why}"),
        }
    }
}

impl std::error::Error for ExportError {}

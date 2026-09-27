//! What can stop an import, in the dialog's words.

use std::fmt;

/// Why an import failed, always with the specific reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// Neither name nor bytes are a readable format.
    Unsupported(String),
    /// Recognised format, bad content; the string says where it stopped making sense.
    Malformed(String),
    /// Parsed, but no triangles.
    Empty,
    Cancelled,
    Io(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Unsupported(what) => write!(
                f,
                "{what}\n\nSimple 3D reads the formats it writes: 3MF, STL (binary or text), OBJ and PLY \
                 (binary or text)."
            ),
            ImportError::Malformed(why) => write!(f, "The file is not readable as the format it claims to be: {why}"),
            ImportError::Empty => write!(f, "The file holds no triangles, so there is nothing to bring in."),
            ImportError::Cancelled => write!(f, "Import cancelled. Nothing was brought in."),
            ImportError::Io(why) => write!(f, "Reading the file failed: {why}"),
        }
    }
}

impl std::error::Error for ImportError {}

pub(crate) fn malformed(why: impl Into<String>) -> ImportError {
    ImportError::Malformed(why.into())
}

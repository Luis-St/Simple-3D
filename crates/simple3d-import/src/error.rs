//! What can stop an import, in the words the dialog shows.

use std::fmt;

/// Why an import did not happen. Every variant carries the specific reason:
/// "the file could not be read" on its own tells a user nothing about a model
/// somebody else's program wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// The name says nothing this crate reads, and neither do the bytes.
    Unsupported(String),
    /// The format was recognised and the content does not hold up. The string
    /// says where it stopped making sense.
    Malformed(String),
    /// A file that parsed, and holds no triangles.
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

/// A parse failure, as the readers report it.
pub(crate) fn malformed(why: impl Into<String>) -> ImportError {
    ImportError::Malformed(why.into())
}

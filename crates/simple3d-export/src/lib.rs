//! Mesh export (spec section 9): 3MF, STL, OBJ and PLY, with binary variants
//! where the format has one.
//!
//! Three things the spec insists on and this module implements:
//!
//! * **Verify before writing.** A mesh that is not watertight, manifold and
//!   consistently wound with outward normals is reported -- naming the node
//!   responsible -- rather than written out to fail in a slicer later.
//! * **No partial file.** Everything is written to a sibling temporary file and
//!   renamed into place only once it is complete, so a cancelled or failed
//!   export leaves nothing behind.
//! * **Cancellable with progress.** The caller passes a callback that reports
//!   progress and returns `false` to cancel.

mod format;
pub use format::{BodyMode, Format, Unit3mf};
mod options;
pub use options::{Options, Progress};
mod error;
pub use error::ExportError;
mod verify;
pub use verify::{signed_volume, verify};
mod write;
pub use write::{write, write_parts, Part};
mod three_mf;
pub(crate) use three_mf::*;
mod stl;
pub(crate) use stl::*;
mod obj;
pub(crate) use obj::*;
mod ply;
pub(crate) use ply::*;
#[cfg(test)]
mod tests;

pub mod zip;

//! Mesh export (spec section 9): 3MF, STL, OBJ and PLY, binary where the format has it.
//!
//! * Verify before writing: a non-watertight, non-manifold or inward-wound mesh is reported with its
//!   node instead of failing later in a slicer.
//! * No partial file: written to a sibling temp file and renamed into place when complete.
//! * Cancellable: the progress callback returns `false` to cancel.

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
mod deflate;
mod stl;
pub(crate) use stl::*;
mod obj;
pub(crate) use obj::*;
mod ply;
pub(crate) use ply::*;
#[cfg(test)]
mod tests;

pub mod zip;

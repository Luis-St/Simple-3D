//! The project file (spec section 10).
//!
//! One user-visible file holding the entire scene, the display unit, the scene
//! settings and the camera. Human-readable and text-based -- pretty-printed JSON
//! with `BTreeMap`-ordered keys -- so projects diff cleanly and can go under
//! version control. Node subtrees use the same `NodeData` schema as the
//! clipboard, so a selection copied to the clipboard can be pasted into a text
//! editor and back again.

mod error;
pub use error::LoadError;
pub(crate) use error::*;
mod io;
pub use io::{from_str, to_string};
mod unknown;
pub(crate) use unknown::*;
#[cfg(test)]
mod tests;

use crate::scene::{Camera, NodeData, SceneSettings};
use serde::{Deserialize, Serialize};

/// Bumped whenever the schema changes in a way an older build could not read.
///
/// Version 2 added the body issue 80 called for: a `mesh` node carrying its own
/// geometry rather than a recipe for it. A version 1 build meeting one would
/// fail on the unknown type rather than open the file half-understood, so the
/// version says so first.
///
/// Version 3 added the other half of issue 82: a `split` node, holding the
/// pieces a shape was broken into and -- in its `original` field -- the shape
/// itself, so the break can be undone long after the fact.
///
/// Cutting a shape into a pattern of cells (issue 82 again) did *not* need a
/// fourth. It writes one more optional field, `tiling`, saying which cell shape
/// and which numbers made the pieces; a build that has never heard of it reads
/// the same file as the split it is -- the pieces are ordinary stored meshes and
/// the shape they came from is where it always was -- and loses only the label
/// on it. A version is bumped for a file an older build could not read, not for
/// one it reads with a word missing.
pub const FORMAT_VERSION: u32 = 3;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format: u32,
    #[serde(default)]
    generator: String,
    settings: SceneSettings,
    camera: Camera,
    root: NodeData,
}

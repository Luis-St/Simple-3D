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
pub use io::{from_str, project_from_str, project_to_string, to_string};
mod unknown;
pub(crate) use unknown::*;
#[cfg(test)]
mod tests;

use crate::scene::{Camera, ComponentId, NodeData, Scene, SceneSettings};
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
///
/// Version 4 added components (issue 113): a project of several node trees,
/// with nodes standing for whole other ones. A version 3 build would read such
/// a file as its root component alone, with every integration in it refused as
/// an unknown type -- so the version says so first. A project that has never
/// used components is still written as version 3, which it is, and an older
/// build opens it as it always did.
pub const FORMAT_VERSION: u32 = 4;

/// The version a project without components is written as -- see
/// [`FORMAT_VERSION`].
pub const PLAIN_FORMAT: u32 = 3;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format: u32,
    #[serde(default)]
    generator: String,
    settings: SceneSettings,
    camera: Camera,
    root: NodeData,
    /// Every component after the root one, which is the rest of the file
    /// (issue 113). Absent for a project that has never used components, which
    /// is then exactly the file it was before they existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    components: Vec<ComponentFile>,
}

/// One component after the root one: a scene of its own, as the top of the
/// file is the root component's.
#[derive(Serialize, Deserialize)]
struct ComponentFile {
    id: ComponentId,
    settings: SceneSettings,
    camera: Camera,
    root: NodeData,
}

/// A whole project: its root component, and every other one by id, in the
/// order they were made.
pub struct ProjectData {
    pub root: Scene,
    pub components: Vec<(ComponentId, Scene)>,
}

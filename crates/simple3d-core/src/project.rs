//! The project file (spec section 10).
//!
//! One file with the scene, unit, settings and camera, as pretty-printed JSON with ordered keys so
//! it diffs cleanly. Subtrees use the clipboard's `NodeData` schema.

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

/// Bumped when the schema changes in a way an older build could not read.
///
/// * 2: `mesh` nodes with their own geometry (issue 80).
/// * 3: `split` nodes holding pieces and the original (issue 82). The later optional `tiling`
///   field needed no bump, since older builds read the file fine without it.
/// * 4: components (issue 113). Projects without components are still written as 3.
pub const FORMAT_VERSION: u32 = 4;

/// The version a project without components is written as (see [`FORMAT_VERSION`]).
pub const PLAIN_FORMAT: u32 = 3;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format: u32,
    #[serde(default)]
    generator: String,
    settings: SceneSettings,
    camera: Camera,
    root: NodeData,
    /// Every component after the root (issue 113); absent for a project that never used components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    components: Vec<ComponentFile>,
}

/// One non-root component: its own scene, stored like the root's at the top of the file.
#[derive(Serialize, Deserialize)]
struct ComponentFile {
    id: ComponentId,
    settings: SceneSettings,
    camera: Camera,
    root: NodeData,
}

/// A whole project: its root component and every other one by id, in creation order.
pub struct ProjectData {
    pub root: Scene,
    pub components: Vec<(ComponentId, Scene)>,
}

//! Several documents open at once, one per tab (issue 61).
//!
//! There is still one current document, read straight off `App` everywhere. Other tabs' state is
//! lifted into a `Document` and put back when picked. The entry at `App::active` is a stale
//! stand-in; only this module reads a `Document`, answering the active tab from `App`.

mod close;
pub(crate) mod drag;
pub use drag::{resolve_drag, TabDrag};
mod open;
pub(crate) mod strip;
mod swap;
mod transfer;
pub use strip::show;

use simple3d_core::eval::Evaluated;
use simple3d_core::scene::Scene;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One open document: everything about the model in a tab (camera included, in `scene`), but not
/// the window's tool mode, dock layout or settings.
pub struct Document {
    /// The document's on-screen component.
    pub model: crate::components::ModelState,
    pub path: Option<PathBuf>,
    /// The rest of the project: every component but the one that was on screen (issue 113).
    pub project: crate::components::Project,
}

impl Document {
    /// An empty, unsaved document: a new tab, or the active tab's stand-in.
    pub fn empty() -> Document {
        Document {
            model: crate::components::ModelState::new(Scene::new()),
            path: None,
            project: crate::components::Project::new(),
        }
    }

    fn unsaved(&self) -> bool {
        self.model.history.revision() != self.model.saved_revision || self.project.unsaved()
    }

    fn name(&self) -> String {
        document_name(self.path.as_deref())
    }
}

/// The result of evaluating nothing, shown before the first evaluation lands.
pub fn empty_evaluation() -> Evaluated {
    Evaluated {
        mesh: std::sync::Arc::new(simple3d_geom::Mesh::new()),
        bounds: None,
        node_meshes: BTreeMap::new(),
        group_meshes: BTreeMap::new(),
        node_frames: BTreeMap::new(),
        node_local_bounds: BTreeMap::new(),
        node_world_bounds: BTreeMap::new(),
        ranges: BTreeMap::new(),
        placements: BTreeMap::new(),
        errors: Vec::new(),
        cancelled: false,
    }
}

/// A document's name: its file name, or `Untitled` before it has one.
pub fn document_name(path: Option<&Path>) -> String {
    match path {
        Some(path) => path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        None => "Untitled".to_string(),
    }
}

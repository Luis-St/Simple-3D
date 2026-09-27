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
use simple3d_core::scene::{NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_geom::Vec3;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// One open document: everything about the model in a tab (camera included, in `scene`), but not
/// the window's tool mode, dock layout or settings.
pub struct Document {
    pub scene: Scene,
    pub history: History,
    pub path: Option<PathBuf>,
    pub saved_revision: u64,
    pub selection: Vec<NodeId>,
    pub selection_anchor: Option<NodeId>,
    pub collapsed: HashSet<NodeId>,
    pub cursor: Option<Vec3>,
    pub frame_when_evaluated: bool,
    /// The last evaluation, so returning to a tab shows the model at once.
    pub evaluated: Evaluated,
    /// The rest of the project: every component but the one that was on screen (issue 113).
    pub project: crate::components::Project,
}

impl Document {
    /// An empty, unsaved document: a new tab, or the active tab's stand-in.
    pub fn empty() -> Document {
        Document {
            scene: Scene::new(),
            history: History::new(),
            path: None,
            saved_revision: 0,
            selection: Vec::new(),
            selection_anchor: None,
            collapsed: HashSet::new(),
            cursor: None,
            frame_when_evaluated: true,
            evaluated: empty_evaluation(),
            project: crate::components::Project::new(),
        }
    }

    fn unsaved(&self) -> bool {
        self.history.revision() != self.saved_revision || self.project.unsaved()
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

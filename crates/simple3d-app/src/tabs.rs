//! Several documents open at once, one per tab (issue 61).
//!
//! The application still has exactly one *current* document, and every panel,
//! command and gesture goes on reading it straight off `App` as it always did.
//! What tabs add is the documents that are not current: their state is lifted
//! off `App` into a `Document` and put back when the tab is picked again, so
//! nothing in the rest of the application has to know how many are open.
//!
//! The invariant the switching rests on: `App::tabs` has one entry per open
//! document, and the entry at `App::active` is a stand-in whose contents are
//! stale -- the live state of that document is the one on `App` itself. Nothing
//! outside this module reads a `Document` directly; the tab bar asks the
//! helpers below, which know to answer for the active tab from `App`.

mod close;
mod open;
mod strip;
mod swap;
pub use strip::show;

use simple3d_core::eval::Evaluated;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_core::undo::History;
use simple3d_geom::Vec3;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// One open document: everything about the model in a tab, and nothing about
/// the window it is shown in. The camera travels inside `scene`, the tool mode,
/// the dock layout and the settings are the application's and stay put when the
/// tab changes.
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
    /// The last evaluation of this scene, so coming back to a tab shows the
    /// model at once rather than an empty viewport while it is recomputed.
    pub evaluated: Evaluated,
}

impl Document {
    /// An empty, unsaved document -- what a new tab starts as, and what stands
    /// in for the active tab while its real state lives on `App`.
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
        }
    }

    fn unsaved(&self) -> bool {
        self.history.revision() != self.saved_revision
    }

    fn name(&self) -> String {
        document_name(self.path.as_deref())
    }
}

/// The result of evaluating nothing: what a document shows before its first
/// evaluation lands.
pub fn empty_evaluation() -> Evaluated {
    Evaluated {
        mesh: std::sync::Arc::new(simple3d_geom::Mesh::new()),
        node_meshes: BTreeMap::new(),
        group_meshes: BTreeMap::new(),
        node_frames: BTreeMap::new(),
        node_local_bounds: BTreeMap::new(),
        node_world_bounds: BTreeMap::new(),
        errors: Vec::new(),
        cancelled: false,
    }
}

/// What a document is called: its file name, or `Untitled` before it has one.
pub fn document_name(path: Option<&Path>) -> String {
    match path {
        Some(path) => path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        None => "Untitled".to_string(),
    }
}

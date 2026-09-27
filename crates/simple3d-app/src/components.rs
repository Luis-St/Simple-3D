//! A project's components, each in its own tab (issue 113).
//!
//! The root component is the project itself; others are placed by integrations and edited in a
//! second tab row, each with its own history. As in `crate::tabs`, the current component's live
//! state is on `App` and its entry in [`Project::components`] is a stale stand-in;
//! [`App::component_scene`] knows to answer from `App`.

mod actions;
pub use actions::ComponentAsk;
mod bring_in;
mod history;
pub(crate) mod strip;
mod switch;

use simple3d_core::eval::Evaluated;
use simple3d_core::scene::{ComponentId, NodeId, Scene, ROOT_COMPONENT};
use simple3d_core::undo::History;
use simple3d_geom::Vec3;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

/// What `App` holds for the model on screen, lifted off it while that model is put away.
pub struct ModelState {
    pub scene: Scene,
    pub history: History,
    pub saved_revision: u64,
    pub selection: Vec<NodeId>,
    pub selection_anchor: Option<NodeId>,
    pub collapsed: HashSet<NodeId>,
    pub cursor: Option<Vec3>,
    pub frame_when_evaluated: bool,
    /// The last evaluation, so returning to the model shows it at once.
    pub evaluated: Evaluated,
}

impl ModelState {
    /// `scene`, never edited or saved.
    pub fn new(scene: Scene) -> ModelState {
        ModelState {
            scene,
            history: History::new(),
            saved_revision: 0,
            selection: Vec::new(),
            selection_anchor: None,
            collapsed: HashSet::new(),
            cursor: None,
            frame_when_evaluated: true,
            evaluated: crate::tabs::empty_evaluation(),
        }
    }

    fn unsaved(&self) -> bool {
        self.history.revision() != self.saved_revision
    }
}

/// One component of a project, while it is not the one on screen.
pub struct Component {
    pub id: ComponentId,
    pub model: ModelState,
}

impl Component {
    /// A component holding `scene`, never edited or saved.
    pub fn new(id: ComponentId, scene: Scene) -> Component {
        Component { id, model: ModelState::new(scene) }
    }

    fn unsaved(&self) -> bool {
        self.model.unsaved()
    }
}

/// Everything about a project beyond the component on screen.
pub struct Project {
    /// Every component, root first then in creation order; the on-screen one is a stand-in.
    pub components: Vec<Component>,
    /// The components open in the second tab row; the root is always first and cannot be closed.
    pub open: Vec<ComponentId>,
    /// The component on screen.
    pub active: ComponentId,
    /// The next component's id, never reused, so an integration never changes meaning.
    pub next_id: ComponentId,
    /// Bumped when components are made, deleted or restored, which no component's history sees.
    pub structure_revision: u64,
    pub saved_structure: u64,
    /// Components an undo removed, kept so a redo restores them.
    pub removed: Vec<Component>,
    /// This project's identity while open, so a paste knows whether the clip came from here (`Clip::origin`).
    pub origin: u64,
}

impl Default for Project {
    fn default() -> Self {
        Project::new()
    }
}

impl Project {
    /// A project of one empty root component.
    pub fn new() -> Project {
        static NEXT_ORIGIN: AtomicU64 = AtomicU64::new(1);
        Project {
            components: vec![Component::new(ROOT_COMPONENT, Scene::new())],
            open: vec![ROOT_COMPONENT],
            active: ROOT_COMPONENT,
            next_id: ROOT_COMPONENT + 1,
            structure_revision: 0,
            saved_structure: 0,
            removed: Vec::new(),
            origin: NEXT_ORIGIN.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// Whether the project has any non-root component, which is when the second tab row shows.
    pub fn uses_components(&self) -> bool {
        self.components.len() > 1
    }

    pub(crate) fn index(&self, id: ComponentId) -> Option<usize> {
        self.components.iter().position(|c| c.id == id)
    }

    pub(crate) fn get(&self, id: ComponentId) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id)
    }

    /// Whether anything but the on-screen component changed since saving; that one is asked of `App`.
    pub fn unsaved(&self) -> bool {
        self.structure_revision != self.saved_structure
            || self.components.iter().any(|c| c.id != self.active && c.unsaved())
    }

    /// A fresh id for a new component.
    pub(crate) fn fresh_id(&mut self) -> ComponentId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

/// A component's name: its root's name.
pub fn component_name(scene: &Scene) -> String {
    scene.node(scene.root()).name.clone()
}

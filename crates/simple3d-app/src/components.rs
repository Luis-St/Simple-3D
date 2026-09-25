//! A project's components, each in a tab of its own (issue 113).
//!
//! A project is several node trees. The first is its root component, which is
//! what a project that has never used components is and all it is; every other
//! one is placed in the root -- or in another component -- by an integration,
//! a single node that stands for the whole of it and shows every edit made to
//! it. A component is edited in a tab of its own, in a second row under the
//! projects', with an undo history of its own, and is drawn there on its own.
//!
//! The invariant is the one `crate::tabs` rests on, one level down: the
//! application has exactly one *current* component, whose state lives on `App`
//! where every panel reads it, and the entry for it in [`Project::components`]
//! is a stand-in whose contents are stale. Nothing outside this module reads a
//! [`Component`] directly for the one on screen; [`App::component_scene`]
//! knows to answer that one from `App`.

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

/// One component of a project, while it is not the one on screen.
pub struct Component {
    pub id: ComponentId,
    pub scene: Scene,
    pub history: History,
    pub saved_revision: u64,
    pub selection: Vec<NodeId>,
    pub selection_anchor: Option<NodeId>,
    pub collapsed: HashSet<NodeId>,
    pub cursor: Option<Vec3>,
    pub frame_when_evaluated: bool,
    pub evaluated: Evaluated,
}

impl Component {
    /// A component holding `scene`, never edited and never saved.
    pub fn new(id: ComponentId, scene: Scene) -> Component {
        Component {
            id,
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

/// Everything about a project beyond the component on screen.
pub struct Project {
    /// Every component, root first and the rest in the order they were made.
    /// The entry for the one on screen is a stand-in.
    pub components: Vec<Component>,
    /// The components open in the second row of tabs, in the order they are
    /// shown. The root component is always first and cannot be closed.
    pub open: Vec<ComponentId>,
    /// The component on screen.
    pub active: ComponentId,
    /// The id the next component made will be given. Never reused within a
    /// project, so an integration can never come to mean a different component
    /// from the one it was made for.
    pub next_id: ComponentId,
    /// Bumped by everything that changes which components there are, which no
    /// component's own history sees: a component made, deleted, or brought
    /// back by a redo.
    pub structure_revision: u64,
    pub saved_structure: u64,
    /// Components an undo has taken away, kept so a redo can bring them back
    /// as they were.
    pub removed: Vec<Component>,
    /// Which project this is while it is open, so a paste can tell whether the
    /// clip came from here -- see `Clip::origin`.
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

    /// Whether the project has any component beyond its root, which is when
    /// the second row of tabs is worth its space.
    pub fn uses_components(&self) -> bool {
        self.components.len() > 1
    }

    pub(crate) fn index(&self, id: ComponentId) -> Option<usize> {
        self.components.iter().position(|c| c.id == id)
    }

    pub(crate) fn get(&self, id: ComponentId) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id)
    }

    /// Whether anything but the component on screen has changed since the
    /// project was saved. The one on screen is asked of `App`.
    pub fn unsaved(&self) -> bool {
        self.structure_revision != self.saved_structure
            || self.components.iter().any(|c| c.id != self.active && c.unsaved())
    }

    /// A fresh id for a component about to be made.
    pub(crate) fn fresh_id(&mut self) -> ComponentId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

/// What a component is called: its root's name, which is what its own tab
/// shows at the top of its tree.
pub fn component_name(scene: &Scene) -> String {
    scene.node(scene.root()).name.clone()
}

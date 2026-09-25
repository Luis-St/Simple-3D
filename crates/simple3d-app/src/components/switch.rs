//! Putting the component on screen away and bringing another out.

use super::*;
use crate::app::{App, Status};
use simple3d_core::scene::Components;
use std::collections::BTreeMap;
use std::sync::Arc;

impl App {
    /// The scene of component `id`, wherever it is: on `App` for the one on
    /// screen, in the project for every other.
    pub(crate) fn component_scene(&self, id: ComponentId) -> Option<&Scene> {
        if id == self.project.active {
            Some(&self.scene)
        } else {
            self.project.get(id).map(|c| &c.scene)
        }
    }

    /// What component `id` is called, or nothing for one the project does not
    /// have.
    pub(crate) fn component_label(&self, id: ComponentId) -> Option<String> {
        self.component_scene(id).map(component_name)
    }

    /// Hand the scene on screen the components it integrates, as they are now.
    ///
    /// Called whenever that could have changed: on coming to a component, and
    /// after anything that makes, deletes or edits one other than the one on
    /// screen. The component on screen is left out of the map -- nothing it
    /// integrates can hold it -- which is also what keeps the map from being
    /// stale about the one scene that changes on every edit.
    pub(crate) fn relink_components(&mut self) {
        let active = self.project.active;
        let scenes: BTreeMap<ComponentId, &Scene> =
            self.project.components.iter().filter(|c| c.id != active).map(|c| (c.id, &c.scene)).collect();
        let linked: Components = simple3d_core::scene::link(&scenes);
        self.scene.components = Arc::new(linked);
        self.dirty = true;
    }

    /// Lift the component on screen off `App`, leaving the project's other
    /// state -- its path, its other components -- where it is.
    fn take_component(&mut self) -> Component {
        self.cancel_simplify_tool();
        Component {
            id: self.project.active,
            scene: std::mem::take(&mut self.scene),
            history: std::mem::take(&mut self.history),
            saved_revision: self.saved_revision,
            selection: std::mem::take(&mut self.selection),
            selection_anchor: self.selection_anchor.take(),
            collapsed: std::mem::take(&mut self.collapsed),
            cursor: self.cursor.take(),
            frame_when_evaluated: self.frame_when_evaluated,
            evaluated: std::mem::replace(&mut self.evaluated, crate::tabs::empty_evaluation()),
        }
    }

    fn put_component(&mut self, component: Component) {
        self.scene = component.scene;
        self.history = component.history;
        self.saved_revision = component.saved_revision;
        self.selection = component.selection;
        self.selection_anchor = component.selection_anchor;
        self.collapsed = component.collapsed;
        self.cursor = component.cursor;
        self.frame_when_evaluated = component.frame_when_evaluated;
        self.evaluated = component.evaluated;
    }

    /// Show component `id` of the project on screen, opening its tab if it is
    /// not open yet.
    pub fn activate_component(&mut self, id: ComponentId) {
        if id == self.project.active {
            return;
        }
        let Some(next) = self.project.index(id) else { return };
        let current = self.take_component();
        let at = self.project.index(current.id).expect("the component on screen is in the project");
        self.project.components[at] = current;
        let next = std::mem::replace(&mut self.project.components[next], Component::new(id, Scene::new()));
        self.project.active = id;
        if !self.project.open.contains(&id) {
            self.project.open.push(id);
        }
        self.put_component(next);
        self.relink_components();
        self.after_switch();
        self.status = Status::Info(format!("Editing {}", component_name(&self.scene)));
    }

    /// Close the tab of component `id`. The component stays in the project --
    /// closing a tab is putting it out of sight, not deleting anything -- and
    /// the root component's tab never closes.
    pub fn close_component(&mut self, id: ComponentId) {
        if id == ROOT_COMPONENT {
            return;
        }
        let Some(at) = self.project.open.iter().position(|&open| open == id) else { return };
        if id == self.project.active {
            // The neighbour to the left: the root is always there to fall back
            // on, so there always is one.
            let next = self.project.open[at.saturating_sub(1)];
            self.activate_component(next);
        }
        self.project.open.retain(|&open| open != id);
    }
}

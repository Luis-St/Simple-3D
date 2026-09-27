//! Putting the on-screen component away and bringing another out.

use super::*;
use crate::app::{App, Status};
use simple3d_core::scene::Components;
use std::collections::BTreeMap;
use std::sync::Arc;

impl App {
    /// Component `id`'s scene: on `App` for the one on screen, in the project otherwise.
    pub(crate) fn component_scene(&self, id: ComponentId) -> Option<&Scene> {
        if id == self.project.active {
            Some(&self.scene)
        } else {
            self.project.get(id).map(|c| &c.scene)
        }
    }

    /// Component `id`'s name, or nothing if the project lacks it.
    pub(crate) fn component_label(&self, id: ComponentId) -> Option<String> {
        self.component_scene(id).map(component_name)
    }

    /// Give the on-screen scene its integrated components as they are now, whenever that may have
    /// changed. The on-screen one is left out, since nothing it integrates can hold it.
    pub(crate) fn relink_components(&mut self) {
        let active = self.project.active;
        let scenes: BTreeMap<ComponentId, &Scene> =
            self.project.components.iter().filter(|c| c.id != active).map(|c| (c.id, &c.scene)).collect();
        let linked: Components = simple3d_core::scene::link(&scenes);
        self.scene.components = Arc::new(linked);
        self.dirty = true;
    }

    /// Lift the on-screen component off `App`, leaving the project's other state.
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

    /// Show component `id`, opening its tab if needed.
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

    /// Close component `id`'s tab without deleting it; the root's tab never closes.
    pub fn close_component(&mut self, id: ComponentId) {
        if id == ROOT_COMPONENT {
            return;
        }
        let Some(at) = self.project.open.iter().position(|&open| open == id) else { return };
        if id == self.project.active {
            // The left neighbour; the root always exists to fall back on.
            let next = self.project.open[at.saturating_sub(1)];
            self.activate_component(next);
        }
        self.project.open.retain(|&open| open != id);
    }
}

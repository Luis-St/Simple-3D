//! Making, placing, opening and deleting components.

use super::*;
use crate::app::{App, Modal, Status};
use simple3d_core::scene::{free_name, is_default_name, reaches, SceneSettings};

/// A component question waiting on a dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentAsk {
    /// Undoing the step that made the component, removing it and everything done to it since.
    Undo(ComponentId),
    /// Deleting the component and every integration of it.
    Delete(ComponentId),
}

impl App {
    /// Whether component `from` may be placed in the on-screen component, and why not if not.
    pub(crate) fn can_integrate(&self, from: ComponentId) -> Result<(), String> {
        let target = self.project.active;
        if from == ROOT_COMPONENT {
            return Err("The root component is the project itself and cannot be placed in anything".into());
        }
        if from == target {
            return Err("A component cannot be placed inside itself".into());
        }
        if self.project.get(from).is_none() {
            return Err("That component is no longer in the project".into());
        }
        let scenes = |id: ComponentId| self.component_scene(id);
        if reaches(&scenes, from, target) {
            let name = self.component_label(from).unwrap_or_default();
            return Err(format!("{name} already has this component inside it"));
        }
        Ok(())
    }

    /// Whether the one selected node is a group that can become a component.
    pub(crate) fn primary_is_group(&self) -> bool {
        self.selection.len() == 1
            && self.primary().is_some_and(|id| id != self.scene.root() && self.scene.node(id).is_group())
    }

    /// Turn the selected group into a component with an integration in its place
    /// (`Command::MakeComponent`). The model looks the same; the contents are now edited in the
    /// component's tab, opened beside this one while the view stays here.
    pub fn make_component(&mut self) {
        let Some(id) = self.primary().filter(|&id| id != self.scene.root() && self.scene.node(id).is_group()) else {
            self.status = Status::Warning("Select a group to make a component of it".into());
            return;
        };
        let component = self.project.fresh_id();
        self.edit("Make component", None);
        let Some(data) = self.scene.make_integration(id, component) else {
            self.history.discard_last();
            self.status = Status::Warning("That group cannot be made a component".into());
            return;
        };
        // A group with its default name gets a component name; a typed name is kept.
        let mut data = data;
        if is_default_name(&data.name, "Group") {
            data.name = self.free_component_name();
            if let Some(node) = self.scene.get_mut(id) {
                node.name = data.name.clone();
            }
        }
        let Some(scene) = Scene::from_root(&data, self.component_settings()) else {
            // Cannot happen, since the subtree came from this scene; if it does, undo is the honest fallback.
            self.history.undo(&mut self.scene);
            self.status = Status::Warning("That group cannot be made a component".into());
            return;
        };
        self.history.mark_created(&[component]);
        let name = component_name(&scene);
        self.project.components.push(Component::new(component, scene));
        self.project.open.push(component);
        self.project.structure_revision += 1;
        self.relink_components();
        self.collapsed.remove(&id);
        self.select_only(id);
        self.status = Status::Info(format!("Made {name} a component; open it to edit what is inside"));
    }

    /// A new component's settings: the same units and detail, but no view state such as sections.
    fn component_settings(&self) -> SceneSettings {
        let mut settings = self.scene.settings.clone();
        settings.section = Default::default();
        settings.more_sections.clear();
        settings
    }

    /// `Component`, `Component 2`, ...: the first name no component uses.
    fn free_component_name(&self) -> String {
        let taken: std::collections::HashSet<String> = self
            .project
            .components
            .iter()
            .map(|c| self.component_scene(c.id).map(component_name).unwrap_or_default())
            .collect();
        free_name(&taken, "Component")
    }

    /// A new component (`Command::NewComponent`): from the selected group if any, else empty and
    /// opened in its own tab.
    pub fn new_component(&mut self) {
        if self.primary_is_group() {
            self.make_component();
            return;
        }
        let name = self.free_component_name();
        let mut scene = Scene::new();
        scene.settings = self.component_settings();
        let root = scene.root();
        if let Some(node) = scene.get_mut(root) {
            node.name = name.clone();
        }
        let id = self.project.fresh_id();
        self.project.components.push(Component::new(id, scene));
        self.project.structure_revision += 1;
        self.activate_component(id);
        self.status = Status::Info(format!("Made {name}"));
    }

    /// Place component `from` on outliner row `at`: inside a group, beside anything else.
    pub fn integrate_component_at(&mut self, at: NodeId, from: ComponentId) {
        let (parent, index) = self.insertion_from_row(at);
        self.place_component(from, parent, index);
    }

    fn place_component(&mut self, from: ComponentId, parent: NodeId, index: usize) {
        if let Err(why) = self.can_integrate(from) {
            self.status = Status::Warning(why);
            return;
        }
        let name = self.component_label(from).unwrap_or_default();
        self.edit("Place component", None);
        let id = self.scene.add_integration(from, &name, parent, index);
        self.stand_clear(&[id]);
        self.collapsed.remove(&parent);
        self.select_only(id);
        self.status = Status::Info(format!("Placed {name}"));
    }

    /// Open the component the integration `id` stands for.
    pub fn open_component_of(&mut self, id: NodeId) {
        let Some(component) = self.scene.component_of(id) else { return };
        if self.project.get(component).is_none() {
            self.status = Status::Warning("The component it places is no longer in the project".into());
            return;
        }
        self.activate_component(component);
    }

    /// Ask before deleting component `id`, which removes all its integrations irreversibly.
    pub fn ask_delete_component(&mut self, id: ComponentId) {
        if id == ROOT_COMPONENT || self.project.get(id).is_none() {
            return;
        }
        self.component_ask = Some(ComponentAsk::Delete(id));
        self.modal = Modal::ConfirmComponent;
    }

    /// How many integrations of `id` exist across the project.
    pub(crate) fn integration_count(&self, id: ComponentId) -> usize {
        self.project
            .components
            .iter()
            .filter_map(|c| self.component_scene(c.id))
            .map(|scene| scene.integrations_of(id).len())
            .sum()
    }

    /// Delete component `id` and every integration of it.
    pub fn delete_component(&mut self, id: ComponentId) {
        if id == ROOT_COMPONENT {
            return;
        }
        let Some(name) = self.component_label(id) else { return };
        if id == self.project.active {
            self.close_component(id);
        }
        let removed = self.strip_integrations(id, "Delete component");
        self.project.components.retain(|c| c.id != id);
        self.project.open.retain(|&open| open != id);
        self.project.structure_revision += 1;
        self.relink_components();
        self.status = Status::Info(match removed {
            0 => format!("Deleted {name}"),
            1 => format!("Deleted {name} and the one place it was used"),
            n => format!("Deleted {name} and the {n} places it was used"),
        });
    }

    /// Remove every integration of `id` from every component, as an undo step in each, and return the
    /// count. Undoing one restores a node for a missing component, which evaluates as nothing and says why.
    pub(super) fn strip_integrations(&mut self, id: ComponentId, label: &str) -> usize {
        let mut removed = 0;
        let here = self.scene.integrations_of(id);
        if !here.is_empty() {
            self.edit(label, None);
            for node in &here {
                self.scene.remove(*node);
            }
            self.selection.retain(|n| self.scene.contains(*n));
            removed += here.len();
        }
        let active = self.project.active;
        for component in self.project.components.iter_mut().filter(|c| c.id != active && c.id != id) {
            let there = component.scene.integrations_of(id);
            if there.is_empty() {
                continue;
            }
            component.history.record(&component.scene, label, None);
            for node in &there {
                component.scene.remove(*node);
            }
            let scene = &component.scene;
            component.selection.retain(|n| scene.contains(*n));
            removed += there.len();
        }
        removed
    }
}

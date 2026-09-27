//! Components arriving from outside the project: pasted, or in a placed saved primitive.

use super::*;
use crate::app::{App, Status};
use simple3d_core::clipboard::Clip;
use simple3d_core::scene::{GroupOp, NodeData};
use std::collections::BTreeMap;

impl App {
    /// Make `clip` meaningful here: components the project lacks are added under fresh ids and the clip
    /// rewritten. A clip from this project keeps its links (paste places the same component again);
    /// from elsewhere it brings copies (issue 113). Also returns the components made, for undo.
    pub(crate) fn bring_in_components(&mut self, clip: &Clip) -> (Clip, Vec<ComponentId>) {
        let mut clip = clip.clone();
        if clip.components.is_empty() {
            return (clip, Vec::new());
        }
        let here = clip.origin == Some(self.project.origin);
        let mut map: BTreeMap<ComponentId, ComponentId> = BTreeMap::new();
        for carried in &clip.components {
            if here && self.project.get(carried.id).is_some() {
                map.insert(carried.id, carried.id);
            } else {
                map.insert(carried.id, self.project.fresh_id());
            }
        }
        let mut made = Vec::new();
        for carried in &clip.components {
            let to = map[&carried.id];
            if to == carried.id && here {
                continue;
            }
            let mut root = carried.root.clone();
            root.remap_components(&map);
            if let Some(scene) = Scene::from_root(&root, carried.settings.clone()) {
                self.project.components.push(Component::new(to, scene));
                made.push(to);
            }
        }
        for node in &mut clip.nodes {
            node.remap_components(&map);
        }
        if !made.is_empty() {
            self.project.structure_revision += 1;
            self.relink_components();
        }
        (clip, made)
    }

    /// Whether every integration in `clip` could be placed in the on-screen component, and why not.
    /// Only a clip from this project can fail.
    pub(crate) fn clip_fits_here(&self, clip: &Clip) -> Result<(), String> {
        if clip.origin != Some(self.project.origin) {
            return Ok(());
        }
        let mut used = std::collections::BTreeSet::new();
        for node in &clip.nodes {
            node.used_components(&mut used);
        }
        used.into_iter().filter(|&id| self.project.get(id).is_some()).try_for_each(|id| self.can_integrate(id))
    }

    /// Place a saved primitive as its own component (issue 113), with one integration where a new shape
    /// would go. Each placement is a separate component.
    pub(crate) fn place_primitive(&mut self, clip: &Clip, name: &str) {
        let (clip, carried) = self.bring_in_components(clip);
        let Some(root) = component_root(&clip.nodes, name) else {
            self.status = Status::Warning(format!("\u{201C}{name}\u{201D} has nothing in it"));
            return;
        };
        let Some(scene) = Scene::from_root(&root, self.scene.settings.clone()) else {
            self.status = Status::Warning(format!("\u{201C}{name}\u{201D} could not be read"));
            return;
        };
        let component = self.project.fresh_id();
        self.edit("Add", None);
        // Undo removes the component made and any carried inside it.
        let made: Vec<ComponentId> = std::iter::once(component).chain(carried).collect();
        self.history.mark_created(&made);
        self.project.components.push(Component::new(component, scene));
        self.project.structure_revision += 1;
        self.relink_components();
        let (parent, index) = self.scene.insertion_point(self.primary());
        let id = self.scene.add_integration(component, name, parent, index);
        self.stand_clear(&[id]);
        self.select_only(id);
        self.status = Status::Info(format!("Added {name}"));
    }
}

/// A new component's root from `nodes`: the single group at the origin, or a union with the first
/// node at the origin.
fn component_root(nodes: &[NodeData], name: &str) -> Option<NodeData> {
    let first = nodes.first()?;
    if let [only] = nodes {
        if only.type_id == "group" {
            let mut root = only.clone();
            root.name = name.to_string();
            root.position = Vec3::ZERO;
            root.rotation = Vec3::ZERO;
            root.scale = Vec3::ONE;
            root.anchor = Default::default();
            root.visible = true;
            root.ghost = false;
            root.export_body = None;
            return Some(root);
        }
    }
    let anchor = first.position;
    let children = nodes
        .iter()
        .map(|node| {
            let mut node = node.clone();
            node.position = node.position - anchor;
            node
        })
        .collect();
    let mut root = simple3d_core::scene::Scene::new()
        .export_subtree(simple3d_core::scene::Scene::new().root())
        .expect("an empty scene's root always exports");
    root.name = name.to_string();
    root.op = Some(GroupOp::Union);
    root.children = children;
    Some(root)
}

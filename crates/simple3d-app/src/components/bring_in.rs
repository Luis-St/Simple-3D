//! Components arriving from outside the project: in a paste, or in a saved
//! primitive placed from the palette.

use super::*;
use crate::app::{App, Status};
use simple3d_core::clipboard::Clip;
use simple3d_core::scene::{GroupOp, NodeData};
use std::collections::BTreeMap;

impl App {
    /// Make `clip` mean something in this project: every component it carries
    /// that the project does not already have becomes one of its own, under a
    /// fresh id, and the clip is rewritten to point at them.
    ///
    /// A clip copied out of this very project keeps meaning the components it
    /// was copied with, which is what makes copy and paste a way of placing a
    /// component twice. From anywhere else -- another project, the palette --
    /// what it carries is a copy, never a link: editing it here changes nothing
    /// there (issue 113).
    ///
    /// Also gives back the components it made, for the step that places the
    /// clip to take away with it when it is undone.
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

    /// Whether every integration in `clip` could be placed in the component on
    /// screen, and why not when one could not.
    ///
    /// Only a clip from this very project can fail: from anywhere else, every
    /// component it carries arrives as a new one, which nothing here holds.
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

    /// Place a saved primitive as a component of its own (issue 113): the
    /// component is made from what was saved, and one integration of it goes
    /// where a new shape would.
    ///
    /// Every placement is its own component. A primitive placed twice is two
    /// components that happen to start out the same, and editing one leaves
    /// the other alone -- which is what taking something off a shelf twice
    /// means.
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
        // Undone, the placement takes with it the component it made and every
        // one that came along inside it.
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

/// The root a component made of `nodes` has: the one group itself when that
/// is all there is, standing at the origin, and otherwise a union of them,
/// moved together so the first stands at the origin.
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

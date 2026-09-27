//! Components (issue 113): a project of several node trees, where a node can stand for another tree.
//!
//! Each component is its own `Scene` in its own tab; the first is the root. Others are placed by
//! an integration ([`Body::Component`]), so edits show in every integration. A scene carries the
//! components it integrates in [`Scene::components`], so evaluation needs nothing else. The map is
//! built from the leaves up, which requires no component to contain itself: [`link`] enforces that
//! for evaluation and [`reaches`] is checked before making an integration.

use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub type ComponentId = u64;

/// The project's first component, into which pre-component files are read.
pub const ROOT_COMPONENT: ComponentId = 0;

/// Every component a scene integrates, directly or indirectly, by id.
pub type Components = BTreeMap<ComponentId, Arc<Scene>>;

impl Scene {
    /// The component `id` stands for, when it is an integration.
    pub fn component_of(&self, id: NodeId) -> Option<ComponentId> {
        self.nodes.get(&id).and_then(Node::component)
    }

    /// The scene of a component this scene integrates, as evaluation sees it.
    pub fn linked_component(&self, component: ComponentId) -> Option<&Arc<Scene>> {
        self.components.get(&component)
    }

    /// Every component this scene integrates directly.
    pub fn used_components(&self) -> BTreeSet<ComponentId> {
        self.nodes.values().filter_map(Node::component).collect()
    }

    /// Every integration of `component` in this scene, in outliner order.
    pub fn integrations_of(&self, component: ComponentId) -> Vec<NodeId> {
        self.depth_first().into_iter().filter(|&id| self.component_of(id) == Some(component)).collect()
    }

    /// Add an integration of `component` under `parent`.
    pub fn add_integration(&mut self, component: ComponentId, name: &str, parent: NodeId, index: usize) -> NodeId {
        let name = self.unique_name(name);
        self.insert_fresh(name, Body::Component { component, op: None }, parent, index)
    }

    /// Turn group `id` into an integration of `component`, returning the group's contents (with its
    /// operation and colour) for the component. The node keeps its id, name, place, transform and
    /// visibility, so the model looks the same. Refuses the root and non-groups.
    pub fn make_integration(&mut self, id: NodeId, component: ComponentId) -> Option<NodeData> {
        if id == self.root || !self.nodes.get(&id)?.is_group() {
            return None;
        }
        let mut data = self.export_subtree(id)?;
        data.position = simple3d_geom::Vec3::ZERO;
        data.rotation = simple3d_geom::Vec3::ZERO;
        data.scale = simple3d_geom::Vec3::ONE;
        data.anchor = Anchor::Centre;
        data.visible = true;
        data.ghost = false;
        data.export_body = None;
        data.extracted = false;
        for child in self.nodes[&id].children.clone() {
            self.remove(child);
        }
        let node = self.nodes.get_mut(&id)?;
        node.colour = None;
        node.segments = None;
        // An integration cannot carry a split-export mark: there is nothing in the tree to reach into.
        if node.export_body == Some(ExportBody::Split) {
            node.export_body = None;
        }
        node.body = Body::Component { component, op: None };
        Some(data)
    }

    /// A scene with one portable subtree as its root, as a new component starts.
    pub fn from_root(data: &NodeData, settings: SceneSettings) -> Option<Scene> {
        let mut scene = Scene::new();
        scene.settings = settings;
        scene.replace_root(data)?;
        Some(scene)
    }

    /// Re-point every integration per `map`, for a subtree whose component ids came from elsewhere.
    pub fn remap_components(&mut self, map: &BTreeMap<ComponentId, ComponentId>) {
        for node in self.nodes.values_mut() {
            if let Body::Component { component, .. } = &mut node.body {
                if let Some(&to) = map.get(component) {
                    *component = to;
                }
            }
        }
    }
}

impl NodeData {
    /// Every component an integration in this subtree stands for.
    pub fn used_components(&self, out: &mut BTreeSet<ComponentId>) {
        if let Some(component) = self.component {
            out.insert(component);
        }
        for child in &self.children {
            child.used_components(out);
        }
    }

    /// [`Scene::remap_components`], for the portable form.
    pub fn remap_components(&mut self, map: &BTreeMap<ComponentId, ComponentId>) {
        if let Some(component) = &mut self.component {
            if let Some(&to) = map.get(component) {
                *component = to;
            }
        }
        for child in &mut self.children {
            child.remap_components(map);
        }
    }
}

/// Whether `from` is or reaches `target`. Must be false before integrating `from` into `target`,
/// or the model would contain itself.
pub fn reaches<'a>(scenes: &impl Fn(ComponentId) -> Option<&'a Scene>, from: ComponentId, target: ComponentId) -> bool {
    let mut seen = BTreeSet::new();
    let mut stack = vec![from];
    while let Some(at) = stack.pop() {
        if at == target {
            return true;
        }
        if !seen.insert(at) {
            continue;
        }
        if let Some(scene) = scenes(at) {
            stack.extend(scene.used_components());
        }
    }
    false
}

/// Every component of a project, each holding the ones it integrates, ready to evaluate alone.
/// Built from the leaves up; a self-containing component (only possible via hand-edited files)
/// is left out where it would close the loop, so it evaluates as missing.
pub fn link(scenes: &BTreeMap<ComponentId, &Scene>) -> Components {
    fn visit(
        id: ComponentId,
        scenes: &BTreeMap<ComponentId, &Scene>,
        done: &mut Components,
        path: &mut Vec<ComponentId>,
    ) {
        if done.contains_key(&id) || path.contains(&id) {
            return;
        }
        let Some(scene) = scenes.get(&id) else { return };
        path.push(id);
        let used = scene.used_components();
        let mut own = Components::new();
        for dep in used {
            visit(dep, scenes, done, path);
            if let Some(linked) = done.get(&dep) {
                own.insert(dep, linked.clone());
                for (&nested, scene) in linked.components.iter() {
                    own.insert(nested, scene.clone());
                }
            }
        }
        path.pop();
        let mut linked = (*scene).clone();
        linked.components = Arc::new(own);
        done.insert(id, Arc::new(linked));
    }
    let mut done = Components::new();
    for &id in scenes.keys() {
        visit(id, scenes, &mut done, &mut Vec::new());
    }
    done
}

//! Components (issue 113): a project is made of several node trees, and a node
//! can stand for a whole other one.
//!
//! Each component is a `Scene` of its own, edited in a tab of its own. The
//! first one is the project's root component; every other one is reached from
//! it -- or from another component -- through an *integration*: a single node,
//! [`Body::Component`], carrying nothing but its own place in the tree and the
//! id of the component it stands for. Everything inside comes from the
//! component, so an edit made to it shows up in every integration of it.
//!
//! The components a scene integrates travel with it, in [`Scene::components`],
//! each already holding the ones *it* integrates. Evaluation therefore needs
//! nothing but the scene it is handed -- a worker thread, an export and an
//! undo snapshot all carry the whole of what they need -- and the map is
//! built from the leaves up, which is only possible because a component may
//! never end up inside itself. [`link`] is where that is enforced for what is
//! evaluated, and [`reaches`] is what the interface asks before it makes an
//! integration.

use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub type ComponentId = u64;

/// The project's first component: the one a project that has never used
/// components consists of, and the one a file written before components
/// existed is read into.
pub const ROOT_COMPONENT: ComponentId = 0;

/// Every component a scene integrates, directly or through another one, by id.
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

    /// Every component this scene integrates directly, without looking inside
    /// them.
    pub fn used_components(&self) -> BTreeSet<ComponentId> {
        self.nodes.values().filter_map(Node::component).collect()
    }

    /// Every integration of `component` in this scene, in outliner order.
    pub fn integrations_of(&self, component: ComponentId) -> Vec<NodeId> {
        self.depth_first().into_iter().filter(|&id| self.component_of(id) == Some(component)).collect()
    }

    /// Add an integration of `component` under `parent`.
    pub fn add_integration(&mut self, component: ComponentId, name: &str, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            name: self.unique_name(name),
            position: simple3d_geom::Vec3::ZERO,
            rotation: simple3d_geom::Vec3::ZERO,
            scale: simple3d_geom::Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Component { component, op: None },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }

    /// Turn the group `id` into an integration of `component`, and give back
    /// what the component has to be made of: the group's contents, with its
    /// operation and its colour, standing at the component's origin.
    ///
    /// The node keeps its id, its name, its place in the tree, its transform
    /// and whether it is shown -- everything that is about *where* the group
    /// is rather than what it is -- so nothing that pointed at it has to be
    /// told anything, and the model looks exactly as it did. Its colour goes
    /// into the component, where it paints what it always painted: the shapes
    /// in the group that carry no colour of their own.
    ///
    /// Refuses the root and anything that is not a group.
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
        // A group taken apart for an export is the one mark an integration
        // cannot carry: there is nothing in the tree to reach into.
        if node.export_body == Some(ExportBody::Split) {
            node.export_body = None;
        }
        node.body = Body::Component { component, op: None };
        Some(data)
    }

    /// A scene made of one portable subtree, as its root -- what a component
    /// made from a group, a saved primitive or a pasted one starts as.
    pub fn from_root(data: &NodeData, settings: SceneSettings) -> Option<Scene> {
        let mut scene = Scene::new();
        scene.settings = settings;
        scene.replace_root(data)?;
        Some(scene)
    }

    /// Point every integration at the component `map` says it now is, for a
    /// subtree brought in from somewhere the ids meant something else.
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

/// Whether `from` has `target` in it anywhere: is `target` itself, or
/// integrates a component that reaches it.
///
/// What has to be false before an integration of `from` is put into `target`,
/// since that would put `target` inside itself -- and a model inside itself is
/// one that never finishes evaluating.
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

/// Every component of a project, each holding in its [`Scene::components`]
/// the ones it integrates -- ready to be evaluated on its own.
///
/// Built from the leaves up. A component that would have to hold itself --
/// which the interface never allows, but a file edited by hand can say -- is
/// left out of the map that would close the loop, so its integration there
/// evaluates as missing and says so rather than never finishing.
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

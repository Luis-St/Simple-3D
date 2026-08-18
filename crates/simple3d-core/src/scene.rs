//! The node tree (spec section 3): nodes, groups, the scene and every
//! structural edit the outliner offers.
//!
//! The tree is an arena of nodes keyed by a stable id that survives save/load,
//! reparenting and undo. `BTreeMap` rather than `HashMap` so iteration order is
//! deterministic, which matters because evaluation must be (section 5.2).

use crate::primitive::{self, Params, PrimitiveSpec};
use crate::unit::Unit;
use serde::{Deserialize, Serialize};
use simple3d_geom::{BooleanOp, Vec3};
use std::collections::BTreeMap;

pub type NodeId = u64;

/// Where a node's origin sits (spec section 3.1). Changing it moves the origin,
/// never the shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    #[default]
    Centre,
    /// Minimum Z at the origin, for standing things on a build plate.
    Base,
}

impl Anchor {
    pub const ALL: [Anchor; 2] = [Anchor::Centre, Anchor::Base];

    pub fn label(self) -> &'static str {
        match self {
            Anchor::Centre => "Centre",
            Anchor::Base => "Base",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupOp {
    #[default]
    Union,
    Difference,
    Intersection,
    Hull,
}

impl GroupOp {
    pub const ALL: [GroupOp; 4] = [GroupOp::Union, GroupOp::Difference, GroupOp::Intersection, GroupOp::Hull];

    pub fn label(self) -> &'static str {
        match self {
            GroupOp::Union => "Union",
            GroupOp::Difference => "Difference",
            GroupOp::Intersection => "Intersection",
            GroupOp::Hull => "Hull",
        }
    }

    /// Difference is the only operation where child order carries meaning.
    pub fn order_matters(self) -> bool {
        self == GroupOp::Difference
    }

    pub fn to_geom(self) -> BooleanOp {
        match self {
            GroupOp::Union => BooleanOp::Union,
            GroupOp::Difference => BooleanOp::Difference,
            GroupOp::Intersection => BooleanOp::Intersection,
            GroupOp::Hull => BooleanOp::Hull,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Group { op: GroupOp },
    Primitive { type_id: String, params: Params },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    /// Millimetres, in the parent's frame.
    pub position: Vec3,
    /// Degrees, applied X then Y then Z.
    pub rotation: Vec3,
    /// A factor per axis, applied in the node's own axes before its rotation.
    ///
    /// Distinct from a *resize*, which rewrites the dimension a shape is defined
    /// by and leaves nothing behind. A scale is a factor the node carries, it
    /// applies to a whole group as readily as to one shape, and it is the only
    /// way to make something a proportion of what it was without touching every
    /// number underneath. `1, 1, 1` is no scale at all.
    pub scale: Vec3,
    pub anchor: Anchor,
    /// Hidden nodes are excluded from evaluation and export entirely.
    pub visible: bool,
    /// Per-object override of the scene's default segment count.
    pub segments: Option<u32>,
    pub body: Body,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
}

impl Node {
    /// The smallest a scale factor may get. Zero collapses a solid into a plane
    /// and negative turns it inside out, and neither is a thing to export.
    pub const MIN_SCALE: f64 = 1e-4;

    /// A scale with every axis clamped into the range that produces a solid.
    pub fn sane_scale(scale: Vec3) -> Vec3 {
        Vec3::new(scale.x.max(Node::MIN_SCALE), scale.y.max(Node::MIN_SCALE), scale.z.max(Node::MIN_SCALE))
    }

    pub fn is_group(&self) -> bool {
        matches!(self.body, Body::Group { .. })
    }

    pub fn group_op(&self) -> Option<GroupOp> {
        match self.body {
            Body::Group { op } => Some(op),
            _ => None,
        }
    }

    pub fn spec(&self) -> Option<&'static PrimitiveSpec> {
        match &self.body {
            Body::Primitive { type_id, .. } => primitive::lookup(type_id),
            _ => None,
        }
    }

    pub fn params(&self) -> Option<&Params> {
        match &self.body {
            Body::Primitive { params, .. } => Some(params),
            _ => None,
        }
    }

    pub fn params_mut(&mut self) -> Option<&mut Params> {
        match &mut self.body {
            Body::Primitive { params, .. } => Some(params),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneSettings {
    pub unit: Unit,
    pub default_segments: u32,
    #[serde(default)]
    pub notes: String,
    pub grid_spacing: f64,
    pub grid_visible: bool,
    /// How far one step of a move or resize goes: the increment a drag snaps to
    /// and one press of a nudge key covers. Its own setting rather than the grid
    /// spacing, which is about what the ground looks like -- 1 mm is the step
    /// most people want and a 1 mm grid is unreadable.
    #[serde(default = "default_snap_step")]
    pub snap_step: f64,
    /// The three origin axes, each on its own. An axis running through the model
    /// is a distraction when it is not the one being worked to.
    #[serde(default = "all_axes")]
    pub axes_visible: [bool; 3],
}

fn default_snap_step() -> f64 {
    1.0
}

fn all_axes() -> [bool; 3] {
    [true; 3]
}

impl Default for SceneSettings {
    fn default() -> Self {
        SceneSettings {
            unit: Unit::Millimetre,
            // 32 segments keeps a 3mm pin smooth and a 2m cylinder acceptable
            // without the user touching the setting (spec section 5.1).
            default_segments: 32,
            notes: String::new(),
            grid_spacing: 10.0,
            grid_visible: true,
            snap_step: default_snap_step(),
            axes_visible: all_axes(),
        }
    }
}

/// Saved with the project (spec section 6.1: "The camera position is part of
/// the saved project").
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub target: Vec3,
    pub distance: f64,
    /// Degrees around Z.
    pub yaw: f64,
    /// Degrees above the XY plane.
    pub pitch: f64,
    pub orthographic: bool,
    pub fov_deg: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { target: Vec3::ZERO, distance: 160.0, yaw: -55.0, pitch: 28.0, orthographic: false, fov_deg: 45.0 }
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    nodes: BTreeMap<NodeId, Node>,
    root: NodeId,
    next_id: NodeId,
    pub settings: SceneSettings,
    pub camera: Camera,
}

impl Default for Scene {
    fn default() -> Self {
        Scene::new()
    }
}

impl Scene {
    pub fn new() -> Scene {
        let root = Node {
            id: 1,
            name: "Scene".to_string(),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            segments: None,
            body: Body::Group { op: GroupOp::Union },
            children: Vec::new(),
            parent: None,
        };
        let mut nodes = BTreeMap::new();
        nodes.insert(1, root);
        Scene { nodes, root: 1, next_id: 2, settings: SceneSettings::default(), camera: Camera::default() }
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes[&self.root].children.is_empty()
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[&id]
    }

    pub fn ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.keys().copied()
    }

    fn fresh_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Depth-first order, root first -- the outliner's order.
    pub fn depth_first(&self) -> Vec<NodeId> {
        let mut out = Vec::with_capacity(self.nodes.len());
        self.push_depth_first(self.root, &mut out);
        out
    }

    fn push_depth_first(&self, id: NodeId, out: &mut Vec<NodeId>) {
        out.push(id);
        for &child in &self.nodes[&id].children {
            self.push_depth_first(child, out);
        }
    }

    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.push_depth_first(id, &mut out);
        out.remove(0);
        out
    }

    pub fn is_ancestor_of(&self, ancestor: NodeId, mut node: NodeId) -> bool {
        while let Some(parent) = self.nodes.get(&node).and_then(|n| n.parent) {
            if parent == ancestor {
                return true;
            }
            node = parent;
        }
        false
    }

    /// Whether a node is actually drawn: hidden itself, or under anything
    /// hidden, and it is not. Hiding a group hides everything in it, so asking
    /// the node's own `visible` flag is not the same question.
    pub fn is_shown(&self, id: NodeId) -> bool {
        let mut at = Some(id);
        while let Some(node) = at.and_then(|id| self.nodes.get(&id)) {
            if !node.visible {
                return false;
            }
            at = node.parent;
        }
        true
    }

    pub fn depth(&self, mut id: NodeId) -> usize {
        let mut d = 0;
        while let Some(parent) = self.nodes.get(&id).and_then(|n| n.parent) {
            d += 1;
            id = parent;
        }
        d
    }

    /// Where a new node goes given the current selection: into it if it is a
    /// group, otherwise directly after it as a sibling (spec sections 7.2, 8.1).
    pub fn insertion_point(&self, selection: Option<NodeId>) -> (NodeId, usize) {
        match selection.and_then(|id| self.nodes.get(&id)) {
            Some(node) if node.is_group() => (node.id, node.children.len()),
            Some(node) => {
                let parent = node.parent.unwrap_or(self.root);
                let index = self.nodes[&parent].children.iter().position(|&c| c == node.id).map_or(0, |i| i + 1);
                (parent, index)
            }
            None => (self.root, self.nodes[&self.root].children.len()),
        }
    }

    /// A name that does not already exist among the parent's children, so the
    /// outliner stays readable. Names are not required to be unique.
    fn unique_name(&self, parent: NodeId, base: &str) -> String {
        let taken: Vec<&str> = self.nodes[&parent].children.iter().map(|c| self.nodes[c].name.as_str()).collect();
        if !taken.contains(&base) {
            return base.to_string();
        }
        for n in 2.. {
            let candidate = format!("{base} {n}");
            if !taken.contains(&candidate.as_str()) {
                return candidate;
            }
        }
        unreachable!()
    }

    pub fn add_primitive(&mut self, type_id: &str, parent: NodeId, index: usize) -> Option<NodeId> {
        let spec = primitive::lookup(type_id)?;
        let id = self.fresh_id();
        let node = Node {
            id,
            name: self.unique_name(parent, spec.label),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            segments: None,
            body: Body::Primitive { type_id: type_id.to_string(), params: spec.default_params() },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        Some(id)
    }

    pub fn add_group(&mut self, op: GroupOp, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            name: self.unique_name(parent, "Group"),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            segments: None,
            body: Body::Group { op },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }

    fn link(&mut self, id: NodeId, parent: NodeId, index: usize) {
        let children = &mut self.nodes.get_mut(&parent).expect("parent exists").children;
        let index = index.min(children.len());
        children.insert(index, id);
        self.nodes.get_mut(&id).unwrap().parent = Some(parent);
    }

    fn unlink(&mut self, id: NodeId) {
        if let Some(parent) = self.nodes[&id].parent {
            if let Some(p) = self.nodes.get_mut(&parent) {
                p.children.retain(|&c| c != id);
            }
        }
    }

    /// Delete a subtree. The root is protected (spec section 7.2).
    pub fn remove(&mut self, id: NodeId) -> bool {
        if id == self.root || !self.nodes.contains_key(&id) {
            return false;
        }
        self.unlink(id);
        for descendant in self.descendants(id) {
            self.nodes.remove(&descendant);
        }
        self.nodes.remove(&id);
        true
    }

    /// Deep copy with fresh identities, inserted directly after the original
    /// (spec section 7.2). Every property is preserved.
    pub fn duplicate(&mut self, id: NodeId) -> Option<NodeId> {
        if id == self.root {
            return None;
        }
        let parent = self.nodes.get(&id)?.parent?;
        let index = self.nodes[&parent].children.iter().position(|&c| c == id)? + 1;
        let data = self.export_subtree(id)?;
        self.import_subtree(&data, parent, index)
    }

    /// Move `id` under `new_parent` at `index`. Refuses to create a cycle and
    /// refuses to move the root (spec section 7.2).
    pub fn reparent(&mut self, id: NodeId, new_parent: NodeId, index: usize) -> Result<(), &'static str> {
        if id == self.root {
            return Err("the scene root cannot be moved");
        }
        if !self.nodes.contains_key(&id) || !self.nodes.contains_key(&new_parent) {
            return Err("no such node");
        }
        if !self.nodes[&new_parent].is_group() {
            return Err("only groups can hold children");
        }
        if new_parent == id || self.is_ancestor_of(id, new_parent) {
            return Err("a node cannot be moved inside itself");
        }
        // Index is interpreted against the target's child list *before* the
        // move, so dragging within one parent lands where the indicator showed.
        let same_parent = self.nodes[&id].parent == Some(new_parent);
        let old_index = self.nodes[&new_parent].children.iter().position(|&c| c == id);
        self.unlink(id);
        let index = match (same_parent, old_index) {
            (true, Some(old)) if old < index => index - 1,
            _ => index,
        };
        self.link(id, new_parent, index);
        Ok(())
    }

    /// Move a node up or down among its siblings. Order is semantic inside a
    /// difference group, so this has to be user-controllable.
    pub fn reorder(&mut self, id: NodeId, delta: isize) -> bool {
        let Some(parent) = self.nodes.get(&id).and_then(|n| n.parent) else { return false };
        let children = &mut self.nodes.get_mut(&parent).unwrap().children;
        let Some(from) = children.iter().position(|&c| c == id) else { return false };
        let to = from as isize + delta;
        if to < 0 || to >= children.len() as isize {
            return false;
        }
        let to = to as usize;
        children.remove(from);
        children.insert(to, id);
        true
    }

    /// Wrap the selection in a new group, preserving relative positions and
    /// order (spec section 7.2: "the single most-used structural operation").
    ///
    /// Only the topmost selected nodes are moved -- selecting a group and one of
    /// its children groups the group, not both. The new group's own position
    /// stays at the origin and children keep their coordinates, which is what
    /// keeps relative positions exactly unchanged.
    pub fn group_selection(&mut self, selection: &[NodeId]) -> Option<NodeId> {
        let mut tops: Vec<NodeId> = selection
            .iter()
            .copied()
            .filter(|&id| id != self.root && self.nodes.contains_key(&id))
            .filter(|&id| !selection.iter().any(|&other| other != id && self.is_ancestor_of(other, id)))
            .collect();
        if tops.is_empty() {
            return None;
        }
        // Group into the first selected node's parent, at its position, keeping
        // the tree's own order rather than click order.
        let parent = self.nodes[&tops[0]].parent?;
        let order = self.depth_first();
        tops.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
        tops.retain(|&id| self.nodes[&id].parent == Some(parent));
        if tops.is_empty() {
            return None;
        }
        let index = self.nodes[&parent].children.iter().position(|c| *c == tops[0])?;
        let group = self.add_group(GroupOp::Union, parent, index);
        for (offset, id) in tops.iter().enumerate() {
            self.reparent(*id, group, offset).ok()?;
        }
        Some(group)
    }

    /// A group's base child in a difference: the first *visible* one (spec
    /// section 3.3). The property editor states this plainly.
    pub fn difference_base(&self, group: NodeId) -> Option<NodeId> {
        self.nodes.get(&group)?.children.iter().copied().find(|c| self.nodes[c].visible)
    }

    // -- portable form (project file and clipboard share one schema) ---------

    pub fn export_subtree(&self, id: NodeId) -> Option<NodeData> {
        let node = self.nodes.get(&id)?;
        let (type_id, op, params) = match &node.body {
            Body::Group { op } => ("group".to_string(), Some(*op), Params::new()),
            Body::Primitive { type_id, params } => (type_id.clone(), None, params.clone()),
        };
        Some(NodeData {
            name: node.name.clone(),
            type_id,
            op,
            position: node.position,
            rotation: node.rotation,
            scale: node.scale,
            anchor: node.anchor,
            visible: node.visible,
            segments: node.segments,
            params,
            children: node.children.iter().filter_map(|&c| self.export_subtree(c)).collect(),
        })
    }

    /// Insert a portable subtree, giving every node a fresh identity. Unknown
    /// primitive types are rejected so a corrupt or newer file cannot produce a
    /// half-loaded scene.
    pub fn import_subtree(&mut self, data: &NodeData, parent: NodeId, index: usize) -> Option<NodeId> {
        let body = if data.type_id == "group" {
            Body::Group { op: data.op.unwrap_or_default() }
        } else {
            let spec = primitive::lookup(&data.type_id)?;
            Body::Primitive { type_id: data.type_id.clone(), params: spec.migrate_params(&data.params) }
        };
        let id = self.fresh_id();
        let node = Node {
            id,
            name: if data.name.is_empty() { "Node".to_string() } else { data.name.clone() },
            position: data.position,
            rotation: data.rotation,
            scale: Node::sane_scale(data.scale),
            anchor: data.anchor,
            visible: data.visible,
            segments: data.segments,
            body,
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        for (i, child) in data.children.iter().enumerate() {
            if self.import_subtree(child, id, i).is_none() {
                self.remove(id);
                return None;
            }
        }
        Some(id)
    }

    /// Replace the whole tree from a portable root, used when loading a project.
    pub fn replace_root(&mut self, data: &NodeData) -> Option<()> {
        let mut fresh = Scene::new();
        fresh.settings = self.settings.clone();
        fresh.camera = self.camera;
        {
            let root = fresh.nodes.get_mut(&fresh.root).unwrap();
            root.name = data.name.clone();
            root.body = Body::Group { op: data.op.unwrap_or_default() };
            root.position = data.position;
            root.rotation = data.rotation;
            root.scale = Node::sane_scale(data.scale);
            root.anchor = data.anchor;
            root.visible = data.visible;
            root.segments = data.segments;
        }
        let root = fresh.root;
        for (i, child) in data.children.iter().enumerate() {
            fresh.import_subtree(child, root, i)?;
        }
        self.nodes = fresh.nodes;
        self.root = fresh.root;
        self.next_id = fresh.next_id;
        Some(())
    }

    /// The effective segment count for a node: its own override, else the
    /// scene default.
    pub fn segments_for(&self, id: NodeId) -> u32 {
        self.nodes.get(&id).and_then(|n| n.segments).unwrap_or(self.settings.default_segments).clamp(3, 512)
    }
}

fn default_true() -> bool {
    true
}

/// The readable, diffable form of a node used by both the project file and the
/// clipboard, so a selection can be pasted into a text editor and back again
/// (spec sections 8.1, 10).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeData {
    pub name: String,
    #[serde(rename = "type")]
    pub type_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<GroupOp>,
    #[serde(default = "Vec3_zero")]
    pub position: Vec3,
    #[serde(default = "Vec3_zero")]
    pub rotation: Vec3,
    /// Left out of the file entirely when it is `1, 1, 1`, which is almost
    /// always, so a project written by this version still diffs cleanly against
    /// one written before scale existed.
    #[serde(default = "Vec3_one", skip_serializing_if = "is_unit_scale")]
    pub scale: Vec3,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<u32>,
    #[serde(default, skip_serializing_if = "Params::is_empty")]
    pub params: Params,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeData>,
}

#[allow(non_snake_case)]
fn Vec3_zero() -> Vec3 {
    Vec3::ZERO
}

#[allow(non_snake_case)]
fn Vec3_one() -> Vec3 {
    Vec3::ONE
}

fn is_unit_scale(scale: &Vec3) -> bool {
    *scale == Vec3::ONE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_at(scene: &mut Scene, parent: NodeId, x: f64) -> NodeId {
        let index = scene.node(parent).children.len();
        let id = scene.add_primitive("box", parent, index).unwrap();
        scene.get_mut(id).unwrap().position = Vec3::new(x, 0.0, 0.0);
        id
    }

    #[test]
    fn add_targets_the_selected_group_or_follows_a_leaf() {
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        let group = scene.add_group(GroupOp::Union, root, 1);

        assert_eq!(scene.insertion_point(Some(group)), (group, 0));
        assert_eq!(scene.insertion_point(Some(a)), (root, 1));
        assert_eq!(scene.insertion_point(None), (root, 2));
    }

    #[test]
    fn duplicate_is_a_deep_copy_with_distinct_identity() {
        // Spec acceptance criterion 20/21.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        let child = box_at(&mut scene, group, 5.0);
        scene.get_mut(child).unwrap().anchor = Anchor::Base;
        scene.get_mut(child).unwrap().segments = Some(64);
        scene.get_mut(child).unwrap().visible = false;

        let copy = scene.duplicate(group).unwrap();
        assert_ne!(copy, group);
        assert_eq!(scene.node(root).children, vec![group, copy]);
        assert_eq!(scene.node(copy).group_op(), Some(GroupOp::Difference));
        let copied_child = scene.node(copy).children[0];
        assert_ne!(copied_child, child);
        assert_eq!(scene.node(copied_child).position, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(scene.node(copied_child).anchor, Anchor::Base);
        assert_eq!(scene.node(copied_child).segments, Some(64));
        assert!(!scene.node(copied_child).visible);

        // Editing the copy leaves the original untouched.
        scene.get_mut(copied_child).unwrap().position = Vec3::new(99.0, 0.0, 0.0);
        assert_eq!(scene.node(child).position, Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn reparent_refuses_cycles_and_protects_the_root() {
        let mut scene = Scene::new();
        let root = scene.root();
        let outer = scene.add_group(GroupOp::Union, root, 0);
        let inner = scene.add_group(GroupOp::Union, outer, 0);

        assert!(scene.reparent(outer, inner, 0).is_err());
        assert!(scene.reparent(outer, outer, 0).is_err());
        assert!(scene.reparent(root, outer, 0).is_err());
        assert!(scene.reparent(inner, root, 0).is_ok());
        assert_eq!(scene.node(inner).parent, Some(root));
    }

    #[test]
    fn reparent_within_one_parent_lands_where_the_indicator_showed() {
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        let b = box_at(&mut scene, root, 1.0);
        let c = box_at(&mut scene, root, 2.0);
        // Drop `a` between `b` and `c`: index 2 in the pre-move list.
        scene.reparent(a, root, 2).unwrap();
        assert_eq!(scene.node(root).children, vec![b, a, c]);
    }

    #[test]
    fn grouping_a_selection_preserves_relative_positions_and_order() {
        // Spec acceptance criterion 10.
        let mut scene = Scene::new();
        let root = scene.root();
        let ids: Vec<NodeId> = (0..5).map(|i| box_at(&mut scene, root, i as f64 * 10.0)).collect();
        let before: Vec<Vec3> = ids.iter().map(|&id| scene.node(id).position).collect();

        let group = scene.group_selection(&ids).unwrap();
        assert_eq!(scene.node(group).children, ids);
        assert_eq!(scene.node(group).position, Vec3::ZERO);
        for (&id, &pos) in ids.iter().zip(before.iter()) {
            assert_eq!(scene.node(id).position, pos);
            assert_eq!(scene.node(id).parent, Some(group));
        }
        assert_eq!(scene.node(root).children, vec![group]);
    }

    #[test]
    fn grouping_ignores_children_of_an_already_selected_group() {
        let mut scene = Scene::new();
        let root = scene.root();
        let outer = scene.add_group(GroupOp::Union, root, 0);
        let child = box_at(&mut scene, outer, 0.0);
        let sibling = box_at(&mut scene, root, 5.0);

        let group = scene.group_selection(&[outer, child, sibling]).unwrap();
        assert_eq!(scene.node(group).children, vec![outer, sibling]);
        assert_eq!(scene.node(child).parent, Some(outer));
    }

    #[test]
    fn deleting_the_root_is_refused_and_subtrees_go_entirely() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let child = box_at(&mut scene, group, 0.0);
        assert!(!scene.remove(root));
        assert!(scene.remove(group));
        assert!(!scene.contains(child));
        assert_eq!(scene.len(), 1);
    }

    #[test]
    fn reorder_moves_within_the_parent_only() {
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        let b = box_at(&mut scene, root, 1.0);
        assert!(scene.reorder(b, -1));
        assert_eq!(scene.node(root).children, vec![b, a]);
        assert!(!scene.reorder(b, -1));
        assert!(scene.reorder(b, 1));
        assert_eq!(scene.node(root).children, vec![a, b]);
    }

    #[test]
    fn difference_base_is_the_first_visible_child() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        let a = box_at(&mut scene, group, 0.0);
        let b = box_at(&mut scene, group, 1.0);
        assert_eq!(scene.difference_base(group), Some(a));
        scene.get_mut(a).unwrap().visible = false;
        assert_eq!(scene.difference_base(group), Some(b));
    }

    #[test]
    fn subtree_round_trips_through_the_portable_form() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Intersection, root, 0);
        let child = box_at(&mut scene, group, 7.0);
        scene.get_mut(child).unwrap().rotation = Vec3::new(0.0, 45.0, 0.0);
        scene.get_mut(child).unwrap().name = "Special".into();

        let data = scene.export_subtree(group).unwrap();
        let json = serde_json::to_string(&data).unwrap();
        let back: NodeData = serde_json::from_str(&json).unwrap();
        let pasted = scene.import_subtree(&back, root, 1).unwrap();
        let pasted_child = scene.node(pasted).children[0];
        assert_eq!(scene.node(pasted).group_op(), Some(GroupOp::Intersection));
        assert_eq!(scene.node(pasted_child).name, "Special");
        assert_eq!(scene.node(pasted_child).rotation, Vec3::new(0.0, 45.0, 0.0));
        assert_eq!(scene.node(pasted_child).position, Vec3::new(7.0, 0.0, 0.0));
    }

    #[test]
    fn importing_an_unknown_primitive_type_leaves_no_partial_subtree() {
        let mut scene = Scene::new();
        let root = scene.root();
        let before = scene.len();
        let data = NodeData {
            name: "Group".into(),
            type_id: "group".into(),
            op: Some(GroupOp::Union),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            segments: None,
            params: Params::new(),
            children: vec![NodeData {
                name: "From the future".into(),
                type_id: "hyperboloid".into(),
                op: None,
                position: Vec3::ZERO,
                rotation: Vec3::ZERO,
                scale: Vec3::ONE,
                anchor: Anchor::Centre,
                visible: true,
                segments: None,
                params: Params::new(),
                children: vec![],
            }],
        };
        assert!(scene.import_subtree(&data, root, 0).is_none());
        assert_eq!(scene.len(), before);
        assert!(scene.node(root).children.is_empty());
    }

    #[test]
    fn ids_are_never_reused_after_deletion() {
        let mut scene = Scene::new();
        let root = scene.root();
        let a = box_at(&mut scene, root, 0.0);
        scene.remove(a);
        let b = box_at(&mut scene, root, 0.0);
        assert_ne!(a, b);
    }
}

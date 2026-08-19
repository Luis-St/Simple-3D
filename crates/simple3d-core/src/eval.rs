//! Scene evaluation (spec section 5.2).
//!
//! Turns the node tree into one mesh. Three properties matter and are all
//! tested here:
//!
//! * **Deterministic.** The same tree always produces the same mesh, so the
//!   cache key can be a hash of the subtree and two runs are comparable.
//! * **Cached per subtree**, invalidated only where the tree actually changed,
//!   so editing one dimension does not re-evaluate the whole scene.
//! * **Cancellable.** Evaluation runs off the interaction path and is
//!   superseded cleanly when the user edits again while one is running.
//!
//! A boolean that cannot be evaluated fails loudly on its own node -- named, so
//! the outliner can show it -- while every other branch still previews. It never
//! emits geometry it knows to be broken.

use crate::primitive::ParamValue;
use crate::scene::{Anchor, Body, GroupOp, NodeId, Scene};
use crate::xform::Xform;
use simple3d_geom::{evaluate_boolean, Mesh, Vec3};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A node-specific evaluation failure, carrying the name so the interface can
/// say which node is at fault rather than showing a generic message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeError {
    pub node: NodeId,
    pub name: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct Evaluated {
    /// The whole scene as one mesh, ready to export.
    pub mesh: Arc<Mesh>,
    /// Each primitive's own mesh in world space, for picking, selection
    /// highlighting and the translucent display of hidden nodes. Hidden nodes are
    /// included so they can be drawn as ghosts.
    pub node_meshes: BTreeMap<NodeId, Arc<Mesh>>,
    /// Each node's *parent* frame in world space, including any anchor shift an
    /// ancestor group applied. A manipulator handle needs this to place itself,
    /// and its inverse to turn a world-space drag back into the parent-frame
    /// coordinates `Node::position` is stored in.
    pub node_frames: BTreeMap<NodeId, Xform>,
    /// Each node's own bounding box in its own frame, after its anchor and before
    /// its rotation and position. This is what the resize handles sit on.
    pub node_local_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    /// Each node's bounding box in world space -- what the property editor
    /// reports as the node's measured size. Present for groups as well as
    /// primitives: a group has no mesh of its own in `node_meshes`, but the
    /// assembly it evaluates to is exactly what a user asking "how big is this"
    /// means.
    pub node_world_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    /// Nodes whose own evaluation failed. Non-empty means export must refuse.
    pub errors: Vec<NodeError>,
    /// Set when the run was superseded by a later edit; the result is partial
    /// and should be discarded.
    pub cancelled: bool,
}

impl Evaluated {
    pub fn error_for(&self, node: NodeId) -> Option<&NodeError> {
        self.errors.iter().find(|e| e.node == node)
    }
}

/// Cheap co-operative cancellation. The UI thread flips this when the user
/// edits again, and the worker abandons the run at the next node boundary.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Cancel {
        Cancel(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Holds the caches between runs. Keys are content hashes, so a node that moved
/// in the tree but did not change still hits.
pub struct Evaluator {
    primitives: BTreeMap<u64, Arc<Mesh>>,
    subtrees: BTreeMap<u64, Arc<SubtreeResult>>,
    /// Bounded so a long editing session cannot grow without limit. Entries are
    /// pure functions of their key, so dropping any of them is always safe.
    pub cache_limit: usize,
}

impl Default for Evaluator {
    fn default() -> Self {
        Evaluator::new()
    }
}

#[derive(Debug)]
struct SubtreeResult {
    /// The subtree's mesh in its *parent's* frame.
    mesh: Arc<Mesh>,
    /// The shift the `Base` anchor applied in the node's own frame, before
    /// rotation. Kept so the per-node world transforms agree with the mesh.
    anchor_offset: Vec3,
    errors: Vec<NodeError>,
}

impl Evaluator {
    pub fn new() -> Evaluator {
        Evaluator { primitives: BTreeMap::new(), subtrees: BTreeMap::new(), cache_limit: 4096 }
    }

    pub fn cached_subtrees(&self) -> usize {
        self.subtrees.len()
    }

    pub fn clear(&mut self) {
        self.primitives.clear();
        self.subtrees.clear();
    }

    fn trim(&mut self) {
        // Nothing here tracks recency: the caches exist to make an edit-to-
        // preview cycle fast, and after a wholesale clear the next run repopulates
        // exactly what the current tree needs.
        if self.subtrees.len() > self.cache_limit {
            self.subtrees.clear();
        }
        if self.primitives.len() > self.cache_limit {
            self.primitives.clear();
        }
    }

    pub fn evaluate(&mut self, scene: &Scene, cancel: &Cancel) -> Evaluated {
        let result = self.subtree(scene, scene.root(), cancel);
        let mut collected = Collected::default();
        self.walk(scene, scene.root(), Xform::IDENTITY, &mut collected, cancel);
        self.trim();
        Evaluated {
            mesh: result.mesh.clone(),
            node_meshes: collected.meshes,
            node_frames: collected.frames,
            node_local_bounds: collected.local_bounds,
            node_world_bounds: collected.world_bounds,
            errors: result.errors.clone(),
            cancelled: cancel.is_cancelled(),
        }
    }

    /// The mesh of a node in its *parent's* frame: generated geometry, then the
    /// anchor, then rotation, then position. Anchoring before rotating is what
    /// makes changing the anchor move only the origin, never the shape.
    fn subtree(&mut self, scene: &Scene, id: NodeId, cancel: &Cancel) -> Arc<SubtreeResult> {
        let key = self.subtree_key(scene, id);
        if let Some(hit) = self.subtrees.get(&key) {
            return hit.clone();
        }
        if cancel.is_cancelled() {
            return Arc::new(SubtreeResult {
                mesh: Arc::new(Mesh::new()),
                anchor_offset: Vec3::ZERO,
                errors: Vec::new(),
            });
        }

        let node = scene.node(id);
        let mut errors: Vec<NodeError> = Vec::new();
        let mut local = match &node.body {
            Body::Primitive { .. } => (*self.primitive_mesh(scene, id)).clone(),
            Body::Group { op } => {
                let mut child_meshes: Vec<Mesh> = Vec::new();
                for &child in &node.children {
                    if !scene.node(child).visible {
                        continue;
                    }
                    let child_result = self.subtree(scene, child, cancel);
                    errors.extend(child_result.errors.iter().cloned());
                    if child_result.mesh.triangle_count() > 0 {
                        child_meshes.push((*child_result.mesh).clone());
                    }
                }
                if cancel.is_cancelled() {
                    return Arc::new(SubtreeResult { mesh: Arc::new(Mesh::new()), anchor_offset: Vec3::ZERO, errors });
                }
                combine(*op, &child_meshes, id, &node.name, &mut errors)
            }
        };

        let anchor_offset = match (node.anchor, local.bounds()) {
            (Anchor::Base, Some((lo, _))) => Vec3::new(0.0, 0.0, -lo.z),
            _ => Vec3::ZERO,
        };
        if anchor_offset.z != 0.0 {
            local = local.translated(anchor_offset);
        }
        // Anchor, then scale, then rotate, then translate -- the same order
        // `Xform::from_pos_rot_scale` composes, so the per-node world frames the
        // manipulator uses and the mesh agree. Scaling *after* the anchor is what
        // keeps a base-anchored shape standing on z = 0 whatever it is scaled by.
        let scale = crate::scene::Node::sane_scale(node.scale);
        if scale != Vec3::ONE {
            local = local.scaled(scale);
        }
        let mesh = local.transformed(node.position, node.rotation);
        let result = Arc::new(SubtreeResult { mesh: Arc::new(mesh), anchor_offset, errors });
        self.subtrees.insert(key, result.clone());
        result
    }

    fn primitive_mesh(&mut self, scene: &Scene, id: NodeId) -> Arc<Mesh> {
        let node = scene.node(id);
        let Body::Primitive { type_id, params } = &node.body else { return Arc::new(Mesh::new()) };
        let Some(spec) = crate::primitive::lookup(type_id) else { return Arc::new(Mesh::new()) };
        let segments = if spec.segmented { scene.segments_for(id) } else { 0 };
        let mut hasher = Hasher64::new();
        type_id.hash(&mut hasher.0);
        hash_params(&mut hasher, params);
        segments.hash(&mut hasher.0);
        let key = hasher.finish();
        if let Some(hit) = self.primitives.get(&key) {
            return hit.clone();
        }
        let mesh = Arc::new((spec.build)(params, segments));
        self.primitives.insert(key, mesh.clone());
        mesh
    }

    /// Walk the tree accumulating each node's world frame, its own mesh in world
    /// space and its local bounding box. One pass, so the transforms the handles
    /// use and the meshes picking uses can never disagree.
    fn walk(&mut self, scene: &Scene, id: NodeId, parent: Xform, out: &mut Collected, cancel: &Cancel) {
        if cancel.is_cancelled() {
            return;
        }
        out.frames.insert(id, parent);
        let node = scene.node(id);
        // The anchor shift happens in the node's own frame, before its rotation,
        // so it composes on the right of the node's own transform.
        let anchor_offset = match node.anchor {
            Anchor::Base => self.subtree(scene, id, cancel).anchor_offset,
            Anchor::Centre => Vec3::ZERO,
        };
        let own = parent.compose(&Xform::from_pos_rot_scale(
            node.position,
            node.rotation,
            crate::scene::Node::sane_scale(node.scale),
        ));
        let shifted = own.compose(&Xform::from_translation(anchor_offset));
        match &node.body {
            Body::Primitive { .. } => {
                let mesh = self.primitive_mesh(scene, id);
                if let Some((lo, hi)) = mesh.bounds() {
                    out.local_bounds.insert(id, (lo + anchor_offset, hi + anchor_offset));
                }
                let world = Arc::new(apply(&shifted, &mesh));
                if let Some(bounds) = world.bounds() {
                    out.world_bounds.insert(id, bounds);
                }
                out.meshes.insert(id, world);
            }
            Body::Group { .. } => {
                let subtree = self.subtree(scene, id, cancel);
                if let Some((lo, hi)) = subtree.mesh.bounds() {
                    // A group's mesh is already in its parent's frame, so undo
                    // the node's own transform to get its local box.
                    let inv = Xform::from_pos_rot_scale(
                        node.position,
                        node.rotation,
                        crate::scene::Node::sane_scale(node.scale),
                    )
                    .inverse();
                    let (a, b) = (inv.point(lo), inv.point(hi));
                    out.local_bounds.insert(id, (a.min(b), a.max(b)));
                    // World bounds are measured over the transformed *points*,
                    // not by transporting the box: rotating a box's corners and
                    // taking their extent would report a group under an angled
                    // ancestor as bigger than it is.
                    if let Some(bounds) = bounds_of(subtree.mesh.positions.iter().map(|&p| parent.point(p))) {
                        out.world_bounds.insert(id, bounds);
                    }
                }
                for &child in &node.children {
                    self.walk(scene, child, shifted, out, cancel);
                }
            }
        }
    }

    /// Content hash of a subtree, including everything that affects geometry
    /// and nothing that does not -- a node's name is not in here, so renaming
    /// costs no re-evaluation.
    fn subtree_key(&self, scene: &Scene, id: NodeId) -> u64 {
        let mut hasher = Hasher64::new();
        self.hash_subtree(scene, id, &mut hasher);
        hasher.finish()
    }

    fn hash_subtree(&self, scene: &Scene, id: NodeId, hasher: &mut Hasher64) {
        let node = scene.node(id);
        hash_vec3(hasher, node.position);
        hash_vec3(hasher, node.rotation);
        hash_vec3(hasher, crate::scene::Node::sane_scale(node.scale));
        (node.anchor == Anchor::Base).hash(&mut hasher.0);
        match &node.body {
            Body::Primitive { type_id, params } => {
                type_id.hash(&mut hasher.0);
                hash_params(hasher, params);
                let spec = crate::primitive::lookup(type_id);
                if spec.map_or(false, |s| s.segmented) {
                    scene.segments_for(id).hash(&mut hasher.0);
                }
            }
            Body::Group { op } => {
                "group".hash(&mut hasher.0);
                (*op as u8).hash(&mut hasher.0);
                for &child in &node.children {
                    if scene.node(child).visible {
                        self.hash_subtree(scene, child, hasher);
                    }
                }
                // Length of the visible child list, so hiding the last child of
                // a union is not confused with having one fewer child.
                node.children.iter().filter(|c| scene.node(**c).visible).count().hash(&mut hasher.0);
            }
        }
    }
}

/// The bounding box of one subtree, in its parent's frame, worked out on the
/// spot instead of waiting for the worker.
///
/// Placing a shape clear of something means knowing how big the shape is, and
/// that has to be known the moment it is added -- the background evaluation of
/// the whole scene comes back long afterwards, and the position has to be
/// written before then or the shape visibly jumps. Building the one subtree
/// costs what that subtree costs, which for a single primitive is nothing much.
pub fn subtree_bounds(scene: &Scene, id: NodeId) -> Option<(Vec3, Vec3)> {
    Evaluator::new().subtree(scene, id, &Cancel::new()).mesh.bounds()
}

fn combine(op: GroupOp, children: &[Mesh], id: NodeId, name: &str, errors: &mut Vec<NodeError>) -> Mesh {
    if children.is_empty() {
        return Mesh::new();
    }
    let result = evaluate_boolean(op.to_geom(), children);
    if let Some(issue) = result.manifold_issue() {
        errors.push(NodeError {
            node: id,
            name: name.to_string(),
            message: format!("{} produced non-manifold geometry: {issue}", op.label()),
        });
        // Fall back to showing the operands side by side rather than the broken
        // boolean. Nothing is silent about it -- the node is named in the
        // outliner and export refuses while the error stands (spec section 5.2)
        // -- and the rest of the scene, including this group's own children,
        // still previews, which returning an empty mesh would prevent.
        let mut fallback = Mesh::new();
        for child in children {
            fallback.append(child);
        }
        return fallback;
    }
    result
}

#[derive(Default)]
struct Collected {
    meshes: BTreeMap<NodeId, Arc<Mesh>>,
    frames: BTreeMap<NodeId, Xform>,
    local_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    world_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
}

fn bounds_of(points: impl Iterator<Item = Vec3>) -> Option<(Vec3, Vec3)> {
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for p in points {
        bounds = Some(match bounds {
            Some((lo, hi)) => (lo.min(p), hi.max(p)),
            None => (p, p),
        });
    }
    bounds
}

fn apply(xf: &Xform, mesh: &Mesh) -> Mesh {
    Mesh { positions: mesh.positions.iter().map(|&p| xf.point(p)).collect(), indices: mesh.indices.clone() }
}

/// `DefaultHasher::new` uses fixed keys (unlike `RandomState`), so the same
/// input hashes the same in every process -- which is what lets the cache key be
/// compared across runs and keeps evaluation reproducible.
struct Hasher64(std::collections::hash_map::DefaultHasher);

impl Hasher64 {
    fn new() -> Hasher64 {
        Hasher64(std::collections::hash_map::DefaultHasher::new())
    }

    fn finish(&self) -> u64 {
        self.0.finish()
    }
}

fn hash_f64(hasher: &mut Hasher64, v: f64) {
    // Normalise -0.0 to 0.0 and NaN to a single pattern so equal values always
    // hash equal.
    let v = if v == 0.0 { 0.0 } else { v };
    if v.is_nan() { u64::MAX } else { v.to_bits() }.hash(&mut hasher.0);
}

fn hash_vec3(hasher: &mut Hasher64, v: Vec3) {
    hash_f64(hasher, v.x);
    hash_f64(hasher, v.y);
    hash_f64(hasher, v.z);
}

fn hash_params(hasher: &mut Hasher64, params: &crate::primitive::Params) {
    for (key, value) in params {
        key.hash(&mut hasher.0);
        match value {
            ParamValue::Length(v) | ParamValue::Angle(v) => hash_f64(hasher, *v),
            ParamValue::Count(v) | ParamValue::Choice(v) => v.hash(&mut hasher.0),
            ParamValue::Bool(b) => b.hash(&mut hasher.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::ParamValue;
    use crate::scene::{Anchor, GroupOp, Scene};

    fn plate(scene: &mut Scene, parent: NodeId) -> NodeId {
        let index = scene.node(parent).children.len();
        scene.add_primitive("plate", parent, index).unwrap()
    }

    fn cylinder(scene: &mut Scene, parent: NodeId, diameter: f64, height: f64) -> NodeId {
        let index = scene.node(parent).children.len();
        let id = scene.add_primitive("cylinder", parent, index).unwrap();
        let params = scene.get_mut(id).unwrap().params_mut().unwrap();
        params.insert("diameter_x".into(), ParamValue::Length(diameter));
        params.insert("diameter_y".into(), ParamValue::Length(diameter));
        params.insert("height".into(), ParamValue::Length(height));
        id
    }

    fn size(mesh: &Mesh) -> Vec3 {
        let (lo, hi) = mesh.bounds().unwrap();
        hi - lo
    }

    #[test]
    fn a_scale_multiplies_a_whole_subtree_and_a_base_anchor_still_stands_on_the_ground() {
        // A scale is the one thing that makes a group a proportion of what it
        // was without touching every dimension underneath it, so it has to reach
        // the children.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let a = scene.add_primitive("box", group, 0).unwrap();
        scene.get_mut(a).unwrap().position = Vec3::new(10.0, 0.0, 0.0);

        let plain = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (lo, hi) = plain.mesh.bounds().unwrap();

        scene.get_mut(group).unwrap().scale = Vec3::new(2.0, 2.0, 2.0);
        let scaled = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (slo, shi) = scaled.mesh.bounds().unwrap();
        assert!((slo - lo * 2.0).length() < 1e-9, "{slo:?} is not twice {lo:?}");
        assert!((shi - hi * 2.0).length() < 1e-9, "{shi:?} is not twice {hi:?}");

        // Non-uniformly, on one axis only.
        scene.get_mut(group).unwrap().scale = Vec3::new(1.0, 3.0, 1.0);
        let stretched = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (tlo, thi) = stretched.mesh.bounds().unwrap();
        assert!((thi.y - hi.y * 3.0).abs() < 1e-9, "the Y axis did not stretch: {thi:?}");
        assert!((thi.x - hi.x).abs() < 1e-9, "the X axis stretched too: {thi:?}");
        assert!((tlo.z - lo.z).abs() < 1e-9);

        // The anchor is applied before the scale, so a base-anchored shape is
        // still standing on z = 0 after being scaled.
        scene.get_mut(group).unwrap().scale = Vec3::ONE;
        scene.get_mut(a).unwrap().anchor = Anchor::Base;
        scene.get_mut(a).unwrap().scale = Vec3::new(1.0, 1.0, 4.0);
        let standing = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (blo, bhi) = standing.mesh.bounds().unwrap();
        assert!(blo.z.abs() < 1e-9, "a base-anchored shape scaled up sank to {}", blo.z);
        assert!((bhi.z - (hi.z - lo.z) * 4.0).abs() < 1e-9, "it did not grow by the factor: {bhi:?}");
    }

    #[test]
    fn a_scale_is_part_of_the_cache_key_and_of_the_world_frames() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let child = scene.add_primitive("box", group, 0).unwrap();
        scene.get_mut(child).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
        let mut evaluator = Evaluator::new();
        let before = evaluator.evaluate(&scene, &Cancel::new());
        let child_centre = |out: &Evaluated| {
            let (lo, hi) = out.node_meshes[&child].bounds().unwrap();
            (lo + hi) * 0.5
        };
        assert!((child_centre(&before) - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-9);

        // The same evaluator, so a stale cache entry would show up here.
        scene.get_mut(group).unwrap().scale = Vec3::new(2.0, 2.0, 2.0);
        let after = evaluator.evaluate(&scene, &Cancel::new());
        assert!(
            (child_centre(&after) - Vec3::new(20.0, 0.0, 0.0)).length() < 1e-9,
            "the child's world mesh ignored its parent's scale: {:?}",
            child_centre(&after)
        );
        assert!(after.mesh.bounds().unwrap().1.x > before.mesh.bounds().unwrap().1.x, "the scene mesh was cached");
    }

    #[test]
    fn a_single_primitive_evaluates_to_its_declared_dimensions() {
        let mut scene = Scene::new();
        let root = scene.root();
        plate(&mut scene, root);
        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        assert!(out.errors.is_empty());
        let s = size(&out.mesh);
        assert!((s.x - 40.0).abs() < 1e-9 && (s.y - 20.0).abs() < 1e-9 && (s.z - 4.0).abs() < 1e-9, "{s:?}");
    }

    #[test]
    fn a_hole_drilled_through_a_plate_is_watertight_and_in_place() {
        // Spec acceptance criterion 4, through the node tree this time.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        plate(&mut scene, group);
        let hole = cylinder(&mut scene, group, 6.0, 20.0);
        scene.get_mut(hole).unwrap().position = Vec3::new(-8.0, 0.0, 0.0);

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        assert!(out.mesh.manifold_issue().is_none());
        let s = size(&out.mesh);
        assert!((s.x - 40.0).abs() < 1e-9 && (s.z - 4.0).abs() < 1e-9, "{s:?}");
        for p in &out.mesh.positions {
            let r = ((p.x + 8.0).powi(2) + p.y * p.y).sqrt();
            assert!(r > 3.0 - 1e-6, "a vertex ended up inside the hole");
        }
    }

    #[test]
    fn hiding_a_difference_child_removes_the_cut_and_nothing_else() {
        // Spec acceptance criterion 9.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        plate(&mut scene, group);
        let hole = cylinder(&mut scene, group, 6.0, 20.0);

        let mut evaluator = Evaluator::new();
        let cut = evaluator.evaluate(&scene, &Cancel::new());
        scene.get_mut(hole).unwrap().visible = false;
        let uncut = evaluator.evaluate(&scene, &Cancel::new());

        assert!(cut.mesh.triangle_count() > uncut.mesh.triangle_count());
        let plain = {
            let mut fresh = Scene::new();
            let root = fresh.root();
            plate(&mut fresh, root);
            Evaluator::new().evaluate(&fresh, &Cancel::new())
        };
        assert_eq!(uncut.mesh.triangle_count(), plain.mesh.triangle_count());
    }

    #[test]
    fn nested_groups_move_as_one() {
        // Spec acceptance criterion 8.
        let mut scene = Scene::new();
        let root = scene.root();
        let outer = scene.add_group(GroupOp::Union, root, 0);
        let inner = scene.add_group(GroupOp::Union, outer, 0);
        plate(&mut scene, inner);

        let mut evaluator = Evaluator::new();
        let before = evaluator.evaluate(&scene, &Cancel::new());
        let (lo_before, _) = before.mesh.bounds().unwrap();
        scene.get_mut(outer).unwrap().position = Vec3::new(100.0, 5.0, -3.0);
        let after = evaluator.evaluate(&scene, &Cancel::new());
        let (lo_after, _) = after.mesh.bounds().unwrap();

        assert_eq!(size(&before.mesh), size(&after.mesh));
        let moved = lo_after - lo_before;
        assert!((moved.x - 100.0).abs() < 1e-9 && (moved.y - 5.0).abs() < 1e-9 && (moved.z + 3.0).abs() < 1e-9);
    }

    #[test]
    fn the_base_anchor_moves_the_origin_not_the_shape() {
        // Spec acceptance criterion 7.
        let mut scene = Scene::new();
        let root = scene.root();
        let id = plate(&mut scene, root);
        let mut evaluator = Evaluator::new();
        let centred = evaluator.evaluate(&scene, &Cancel::new());
        scene.get_mut(id).unwrap().anchor = Anchor::Base;
        let based = evaluator.evaluate(&scene, &Cancel::new());

        assert_eq!(size(&centred.mesh), size(&based.mesh));
        assert!((based.mesh.bounds().unwrap().0.z).abs() < 1e-9);
        assert!((centred.mesh.bounds().unwrap().0.z + 2.0).abs() < 1e-9);
    }

    #[test]
    fn evaluation_is_deterministic() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        plate(&mut scene, group);
        cylinder(&mut scene, group, 6.0, 20.0);

        let a = Evaluator::new().evaluate(&scene, &Cancel::new());
        let b = Evaluator::new().evaluate(&scene, &Cancel::new());
        assert_eq!(a.mesh.indices, b.mesh.indices);
        assert_eq!(a.mesh.positions, b.mesh.positions);
    }

    #[test]
    fn editing_one_dimension_reuses_the_rest_of_the_cache() {
        // Spec section 5.2: "invalidated only where the tree actually changed".
        let mut scene = Scene::new();
        let root = scene.root();
        let untouched = scene.add_group(GroupOp::Difference, root, 0);
        plate(&mut scene, untouched);
        cylinder(&mut scene, untouched, 6.0, 20.0);
        let edited = scene.add_group(GroupOp::Union, root, 1);
        let target = cylinder(&mut scene, edited, 10.0, 10.0);
        scene.get_mut(edited).unwrap().position = Vec3::new(80.0, 0.0, 0.0);

        let mut evaluator = Evaluator::new();
        evaluator.evaluate(&scene, &Cancel::new());
        let key_untouched = evaluator.subtree_key(&scene, untouched);
        let key_edited = evaluator.subtree_key(&scene, edited);

        scene.get_mut(target).unwrap().params_mut().unwrap().insert("height".into(), ParamValue::Length(11.0));
        assert_eq!(evaluator.subtree_key(&scene, untouched), key_untouched, "untouched subtree was invalidated");
        assert_ne!(evaluator.subtree_key(&scene, edited), key_edited, "edited subtree was not invalidated");
        assert!(evaluator.subtrees.contains_key(&key_untouched), "untouched subtree fell out of the cache");
    }

    #[test]
    fn renaming_does_not_invalidate_anything() {
        let mut scene = Scene::new();
        let root = scene.root();
        let id = plate(&mut scene, root);
        let evaluator = Evaluator::new();
        let before = evaluator.subtree_key(&scene, root);
        scene.get_mut(id).unwrap().name = "Something else".into();
        assert_eq!(evaluator.subtree_key(&scene, root), before);
    }

    #[test]
    fn a_failing_boolean_names_the_offending_node() {
        // Spec section 5.2: "must fail loudly on that node, naming it in the
        // outliner". A lone triangle is not a solid, so unioning it with itself
        // cannot produce a manifold result -- a deterministic stand-in for the
        // degenerate input a user might build.
        let mut sliver = Mesh::new();
        sliver.push_triangle(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 0.0));
        let mut errors = Vec::new();
        let out = combine(GroupOp::Union, &[sliver.clone(), sliver], 42, "Bad group", &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].node, 42);
        assert_eq!(errors[0].name, "Bad group");
        assert!(errors[0].message.contains("Union"), "{}", errors[0].message);
        // The operands still come back so the rest of the scene can preview.
        assert!(out.triangle_count() > 0);
    }

    #[test]
    fn a_failing_boolean_does_not_stop_the_rest_of_the_scene_previewing() {
        // Spec acceptance criterion 15. A needle-thin operand is the kind of
        // input the epsilon-based kernel cannot resolve; whether it fails is up
        // to the kernel, but if it does the healthy plate must still be there
        // and every reported error must name a real node.
        let mut scene = Scene::new();
        let root = scene.root();
        let good = plate(&mut scene, root);
        scene.get_mut(good).unwrap().position = Vec3::new(200.0, 0.0, 0.0);
        let bad = scene.add_group(GroupOp::Hull, root, 1);
        let needle = cylinder(&mut scene, bad, 1e-4, 10.0);
        scene.get_mut(needle).unwrap().segments = Some(8);

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        for error in &out.errors {
            assert!(scene.contains(error.node), "error names a node that does not exist");
            assert!(!error.message.is_empty());
            assert_eq!(error.name, scene.node(error.node).name);
        }
        let (_, hi) = out.mesh.bounds().unwrap();
        assert!(hi.x > 180.0, "the healthy plate is missing from the preview");
        let _ = bad;
    }

    #[test]
    fn cancelling_abandons_the_run() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        plate(&mut scene, group);
        cylinder(&mut scene, group, 6.0, 20.0);

        let cancel = Cancel::new();
        cancel.cancel();
        let out = Evaluator::new().evaluate(&scene, &cancel);
        assert!(out.cancelled);
    }

    #[test]
    fn per_node_meshes_land_in_world_space() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        scene.get_mut(group).unwrap().position = Vec3::new(50.0, 0.0, 0.0);
        let id = plate(&mut scene, group);
        scene.get_mut(id).unwrap().position = Vec3::new(0.0, 10.0, 0.0);

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let mesh = out.node_meshes.get(&id).expect("primitive mesh missing");
        let (lo, hi) = mesh.bounds().unwrap();
        let centre = (lo + hi) * 0.5;
        assert!((centre.x - 50.0).abs() < 1e-9 && (centre.y - 10.0).abs() < 1e-9, "{centre:?}");
    }

    #[test]
    fn a_nodes_frame_places_its_origin_and_orients_its_axes() {
        // What a manipulator handle relies on: `node_frames[id]` is the *parent*
        // frame, so the node's origin is `frame.point(node.position)` and its
        // own axes come from composing its rotation on top.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        scene.get_mut(group).unwrap().position = Vec3::new(50.0, 0.0, 0.0);
        scene.get_mut(group).unwrap().rotation = Vec3::new(0.0, 0.0, 90.0);
        let id = plate(&mut scene, group);
        scene.get_mut(id).unwrap().position = Vec3::new(10.0, 0.0, 0.0);

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let frame = out.node_frames[&id];
        let origin = frame.point(scene.node(id).position);
        // The group's 90-degree Z rotation turns the child's local +X into +Y.
        assert!((origin - Vec3::new(50.0, 10.0, 0.0)).length() < 1e-9, "{origin:?}");

        // And the world-space mesh agrees with that origin.
        let (lo, hi) = out.node_meshes[&id].bounds().unwrap();
        let centre = (lo + hi) * 0.5;
        assert!((centre - origin).length() < 1e-9, "{centre:?} vs {origin:?}");

        // Turning a world-space drag back into parent-frame coordinates.
        let dragged_to = Vec3::new(50.0, 25.0, 0.0);
        let new_position = frame.inverse().point(dragged_to);
        assert!((new_position - Vec3::new(25.0, 0.0, 0.0)).length() < 1e-9, "{new_position:?}");
    }

    #[test]
    fn local_bounds_follow_the_anchor_and_ignore_the_transform() {
        let mut scene = Scene::new();
        let root = scene.root();
        let id = plate(&mut scene, root);
        scene.get_mut(id).unwrap().position = Vec3::new(100.0, 200.0, 300.0);
        scene.get_mut(id).unwrap().rotation = Vec3::new(0.0, 90.0, 0.0);

        let mut evaluator = Evaluator::new();
        let centred = evaluator.evaluate(&scene, &Cancel::new());
        let (lo, hi) = centred.node_local_bounds[&id];
        assert!((hi - lo - Vec3::new(40.0, 20.0, 4.0)).length() < 1e-9, "{:?}", hi - lo);
        assert!((lo.z + 2.0).abs() < 1e-9, "centre anchor should straddle zero, got {}", lo.z);

        scene.get_mut(id).unwrap().anchor = Anchor::Base;
        let based = evaluator.evaluate(&scene, &Cancel::new());
        let (lo, hi) = based.node_local_bounds[&id];
        assert!(lo.z.abs() < 1e-9, "base anchor should put the local minimum at zero, got {}", lo.z);
        assert!((hi - lo - Vec3::new(40.0, 20.0, 4.0)).length() < 1e-9);
    }

    #[test]
    fn a_group_frame_carries_its_ancestors_anchor_shift() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        scene.get_mut(group).unwrap().anchor = Anchor::Base;
        let id = plate(&mut scene, group);

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        // The group's base anchor lifted its contents by half the plate's
        // thickness, and the child's frame has to know that or its handles would
        // sit below the geometry.
        let origin = out.node_frames[&id].point(scene.node(id).position);
        assert!((origin.z - 2.0).abs() < 1e-9, "{origin:?}");
        let (lo, _) = out.node_meshes[&id].bounds().unwrap();
        assert!(lo.z.abs() < 1e-9, "{lo:?}");
    }

    #[test]
    fn a_group_measures_the_assembly_it_evaluates_to() {
        // A group owns no mesh of its own, so the property editor used to report
        // "no geometry yet" for every group in the scene, forever. Its measured
        // size is the size of what it evaluates to, in world space.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        scene.get_mut(group).unwrap().position = Vec3::new(100.0, 0.0, 0.0);
        plate(&mut scene, group);
        let hole = cylinder(&mut scene, group, 6.0, 20.0);
        scene.get_mut(hole).unwrap().position = Vec3::new(-8.0, 0.0, 0.0);

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (lo, hi) = out.node_world_bounds[&group];
        assert!((hi - lo - Vec3::new(40.0, 20.0, 4.0)).length() < 1e-9, "{:?}", hi - lo);
        // In world space, so the group's own position is included.
        assert!(((lo.x + hi.x) / 2.0 - 100.0).abs() < 1e-9, "{lo:?}");
        // And the root, which is a group too.
        assert!(out.node_world_bounds.contains_key(&root));
    }

    #[test]
    fn a_rotated_group_is_measured_over_its_geometry_not_its_box() {
        // Transporting a group's local box by rotating its eight corners would
        // report a 40mm plate turned 45 degrees as 42mm across -- the box's
        // diagonal, not the plate's. The measurement has to come from the
        // geometry.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        plate(&mut scene, group);
        scene.get_mut(group).unwrap().rotation = Vec3::new(0.0, 0.0, 90.0);

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (lo, hi) = out.node_world_bounds[&group];
        // Turned a quarter turn about Z, the 40 x 20 plate measures 20 x 40.
        assert!((hi - lo - Vec3::new(20.0, 40.0, 4.0)).length() < 1e-9, "{:?}", hi - lo);
    }

    #[test]
    fn hidden_nodes_are_excluded_from_the_result_entirely() {
        let mut scene = Scene::new();
        let root = scene.root();
        let a = plate(&mut scene, root);
        let b = plate(&mut scene, root);
        scene.get_mut(b).unwrap().position = Vec3::new(500.0, 0.0, 0.0);
        scene.get_mut(b).unwrap().visible = false;

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (_, hi) = out.mesh.bounds().unwrap();
        assert!(hi.x < 100.0, "a hidden node contributed geometry");
        // Its own mesh is still available so it can be drawn as a ghost.
        assert!(out.node_meshes.contains_key(&b));
        let _ = a;
    }
}

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
use scadstudio_geom::{evaluate_boolean, Mesh, Vec3};
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
    /// Each visible primitive's own mesh in world space, for picking, selection
    /// highlighting and the translucent display of hidden nodes.
    pub node_meshes: BTreeMap<NodeId, Arc<Mesh>>,
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
        let mut node_meshes: BTreeMap<NodeId, Arc<Mesh>> = BTreeMap::new();
        self.collect_node_meshes(scene, scene.root(), Transform::identity(), &mut node_meshes, cancel);
        self.trim();
        Evaluated {
            mesh: result.mesh.clone(),
            node_meshes,
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
                    return Arc::new(SubtreeResult {
                        mesh: Arc::new(Mesh::new()),
                        anchor_offset: Vec3::ZERO,
                        errors,
                    });
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

    /// World-space mesh per primitive, used for picking, selection highlighting
    /// and drawing hidden nodes as ghosts. Hidden nodes are included -- the
    /// caller knows they took no part in the evaluated mesh.
    fn collect_node_meshes(
        &mut self,
        scene: &Scene,
        id: NodeId,
        parent: Transform,
        out: &mut BTreeMap<NodeId, Arc<Mesh>>,
        cancel: &Cancel,
    ) {
        if cancel.is_cancelled() {
            return;
        }
        let node = scene.node(id);
        // The anchor shift happens in the node's own frame, before its rotation,
        // so it composes on the right of the node's own transform.
        let anchor_offset = match node.anchor {
            Anchor::Base => self.subtree(scene, id, cancel).anchor_offset,
            Anchor::Centre => Vec3::ZERO,
        };
        let here = parent
            .compose(&Transform::from_pos_rot(node.position, node.rotation))
            .compose(&Transform::from_translation(anchor_offset));
        match &node.body {
            Body::Primitive { .. } => {
                let mesh = self.primitive_mesh(scene, id);
                out.insert(id, Arc::new(here.apply(&mesh)));
            }
            Body::Group { .. } => {
                for &child in &node.children {
                    self.collect_node_meshes(scene, child, here, out, cancel);
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

/// An affine transform: rotate by the node's Euler angles, then translate.
/// Composing these is what lets a group's transform apply to a whole assembly
/// while each descendant keeps its own coordinates. A matrix rather than a
/// stack of Euler triples, so nesting depth is unbounded.
#[derive(Clone, Copy, Debug)]
struct Transform {
    /// Row-major 3x3 rotation.
    m: [[f64; 3]; 3],
    t: Vec3,
}

impl Transform {
    fn identity() -> Transform {
        Transform { m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: Vec3::ZERO }
    }

    fn from_translation(t: Vec3) -> Transform {
        Transform { t, ..Transform::identity() }
    }

    /// Built by rotating the basis vectors, so it is the same rotation
    /// `Vec3::rotate_xyz_deg` performs -- X, then Y, then Z -- by construction
    /// rather than by a hand-derived product that could drift from it.
    fn from_pos_rot(position: Vec3, rotation: Vec3) -> Transform {
        let x = Vec3::new(1.0, 0.0, 0.0).rotate_xyz_deg(rotation);
        let y = Vec3::new(0.0, 1.0, 0.0).rotate_xyz_deg(rotation);
        let z = Vec3::new(0.0, 0.0, 1.0).rotate_xyz_deg(rotation);
        Transform { m: [[x.x, y.x, z.x], [x.y, y.y, z.y], [x.z, y.z, z.z]], t: position }
    }

    fn point(&self, p: Vec3) -> Vec3 {
        Vec3::new(
            self.m[0][0] * p.x + self.m[0][1] * p.y + self.m[0][2] * p.z,
            self.m[1][0] * p.x + self.m[1][1] * p.y + self.m[1][2] * p.z,
            self.m[2][0] * p.x + self.m[2][1] * p.y + self.m[2][2] * p.z,
        ) + self.t
    }

    /// `self` applied after `inner`: the result maps `inner`'s local space all
    /// the way out through `self`.
    fn compose(&self, inner: &Transform) -> Transform {
        let mut m = [[0.0; 3]; 3];
        for r in 0..3 {
            for c in 0..3 {
                m[r][c] = (0..3).map(|k| self.m[r][k] * inner.m[k][c]).sum();
            }
        }
        Transform { m, t: self.point(inner.t) }
    }

    fn apply(&self, mesh: &Mesh) -> Mesh {
        Mesh {
            positions: mesh.positions.iter().map(|&p| self.point(p)).collect(),
            indices: mesh.indices.clone(),
        }
    }
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

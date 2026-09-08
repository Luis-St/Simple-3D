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
use crate::scene::{Anchor, Body, ExportBody, GroupOp, NodeId, Scene};
use crate::xform::Xform;
use simple3d_geom::{evaluate_boolean, evaluate_boolean_until, Mesh, Vec3};
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
    /// What each *group* evaluates to, in the frame `node_frames` records for
    /// it -- its own boolean result, not its operands.
    ///
    /// A group has no surface of its own to click on, which is why it is not in
    /// `node_meshes`, but it does have a shape, and the selection outline has to
    /// draw that shape rather than the shapes that went into it. Held
    /// untransformed and shared with the subtree cache, so recording it costs an
    /// `Arc` rather than a copy of the geometry.
    pub group_meshes: BTreeMap<NodeId, Arc<Mesh>>,
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

    /// A node's own result in world space: what the node *is*, booleans and all.
    ///
    /// This is what the selection outline draws. Outlining a group by outlining
    /// its children instead draws shapes the result does not contain -- a
    /// difference's cutter as two rims hanging in mid-air where nothing is, an
    /// intersection's whole uncut box as a cage around the small lens it
    /// actually leaves.
    pub fn result_mesh(&self, id: NodeId) -> Option<std::borrow::Cow<'_, Mesh>> {
        if let Some(mesh) = self.node_meshes.get(&id) {
            return Some(std::borrow::Cow::Borrowed(mesh));
        }
        let mesh = self.group_meshes.get(&id)?;
        let frame = self.node_frames.get(&id)?;
        Some(std::borrow::Cow::Owned(Mesh {
            positions: mesh.positions.iter().map(|&p| frame.point(p)).collect(),
            indices: mesh.indices.clone(),
            tags: mesh.tags.clone(),
        }))
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
            group_meshes: collected.group_meshes,
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
            Body::Mesh { mesh } => {
                let mut copy = mesh.mesh.clone();
                if let Some(colour) = scene.effective_colour(id) {
                    copy.set_tag(colour.tag());
                }
                copy
            }
            Body::Primitive { .. } => {
                // The generated mesh is cached by its shape alone and shared
                // between identical primitives, so the colour is stamped on the
                // copy this node keeps, never on the cached original.
                let mut mesh = (*self.primitive_mesh(scene, id)).clone();
                mesh.set_tag(crate::scene::colour_tag(scene.effective_colour(id)));
                mesh
            }
            // A group and a split are one arm: both are their children
            // combined, and they differ only in the operation, which for a
            // split is always the union its pieces already stood in (issue 82).
            Body::Group { .. } | Body::Split { .. } => {
                let op = node.combine_op().unwrap_or_default();
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
                combine(op, &child_meshes, id, &node.name, &mut errors, cancel)
            }
            Body::Pattern { params } => {
                // The unit the pattern repeats: its children, placed by their own
                // positions and appended. A pattern lays copies side by side, it
                // does not boolean them, so this is a concatenation and stays fast
                // however many copies there are.
                let mut unit = Mesh::new();
                for &child in &node.children {
                    if !scene.node(child).visible {
                        continue;
                    }
                    let child_result = self.subtree(scene, child, cancel);
                    errors.extend(child_result.errors.iter().cloned());
                    unit.append(&child_result.mesh);
                }
                if cancel.is_cancelled() {
                    return Arc::new(SubtreeResult { mesh: Arc::new(Mesh::new()), anchor_offset: Vec3::ZERO, errors });
                }
                let mut copies: Vec<Mesh> = Vec::new();
                for instance in crate::pattern::instances(params) {
                    // Checked per copy, not just before the loop: a pattern is
                    // the one node whose cost is a number someone types, so a
                    // count that turns out to be too large has to be abandonable
                    // rather than run to the end.
                    if cancel.is_cancelled() {
                        return Arc::new(SubtreeResult {
                            mesh: Arc::new(Mesh::new()),
                            anchor_offset: Vec3::ZERO,
                            errors,
                        });
                    }
                    let mut copy = apply(&instance.xform, &unit);
                    // A reflection reverses the winding, so its faces point the
                    // wrong way until they are flipped back.
                    if instance.mirrored {
                        copy.flip_winding();
                    }
                    copies.push(copy);
                }
                // The copies are *unioned*, not concatenated. A pattern stands in
                // for manual duplicates, so it has to produce what those would:
                // duplicates dropped in a union group meet the kernel as separate
                // operands and come out one clean solid, and copies that merely
                // touch -- which is what a step equal to the shape's own width
                // gives -- must do the same. Concatenating them instead welded
                // the contact into an edge shared by four triangles, and the
                // enclosing group then reported the whole scene non-manifold.
                //
                // This costs nothing for the copies that stand clear of each
                // other, which is the ordinary case and the one a helix makes
                // many of: `union_all` rejects non-overlapping operands on their
                // bounding boxes and never enters the BSP kernel for them.
                combine(GroupOp::Union, &copies, id, &node.name, &mut errors, cancel)
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
        // Nothing computed under cancellation is kept, however finished it
        // looks. An abandoned boolean gives back an empty mesh, and the
        // evaluator -- and so this cache -- outlives the run that was
        // abandoned: cached, that empty mesh is what every later evaluation of
        // the same content gets back, so the shapes vanish from the viewport
        // and stay vanished until something changes the content hash. The
        // cancelled run's own answer is dropped by the worker; this is the
        // other half of dropping it.
        if !cancel.is_cancelled() {
            self.subtrees.insert(key, result.clone());
        }
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
                let mut world = apply(&shifted, &mesh);
                world.set_tag(crate::scene::colour_tag(scene.effective_colour(id)));
                let world = Arc::new(world);
                if let Some(bounds) = world.bounds() {
                    out.world_bounds.insert(id, bounds);
                }
                out.meshes.insert(id, world);
            }
            // A stored mesh: its own geometry in world space, so picking, the
            // selection outline and the ghost display all reach it.
            Body::Mesh { .. } => {
                let subtree = self.subtree(scene, id, cancel);
                let inv =
                    Xform::from_pos_rot_scale(node.position, node.rotation, crate::scene::Node::sane_scale(node.scale))
                        .inverse();
                if let Some((lo, hi)) = subtree.mesh.bounds() {
                    let (a, b) = (inv.point(lo), inv.point(hi));
                    out.local_bounds.insert(id, (a.min(b), a.max(b)));
                }
                let world = Arc::new(apply(&parent, &subtree.mesh));
                if let Some(bounds) = world.bounds() {
                    out.world_bounds.insert(id, bounds);
                }
                out.meshes.insert(id, world);
            }
            // A split is walked as the group it behaves like: its pieces are
            // its children, and the node itself outlines what they make.
            Body::Group { .. } | Body::Split { .. } => {
                let subtree = self.subtree(scene, id, cancel);
                // The group's own result, kept so the selection outline can draw
                // the shape the group *is*. It stays in the parent's frame:
                // `parent` is what `node_frames` records for this node, so the
                // pair is enough to place it, and sharing the `Arc` with the
                // subtree cache costs nothing.
                out.group_meshes.insert(id, subtree.mesh.clone());
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
            Body::Pattern { .. } => {
                // Bounds like a group, over the whole repeated result.
                let subtree = self.subtree(scene, id, cancel);
                if let Some((lo, hi)) = subtree.mesh.bounds() {
                    let inv = Xform::from_pos_rot_scale(
                        node.position,
                        node.rotation,
                        crate::scene::Node::sane_scale(node.scale),
                    )
                    .inverse();
                    let (a, b) = (inv.point(lo), inv.point(hi));
                    out.local_bounds.insert(id, (a.min(b), a.max(b)));
                    if let Some(bounds) = bounds_of(subtree.mesh.positions.iter().map(|&p| parent.point(p))) {
                        out.world_bounds.insert(id, bounds);
                    }
                }
                // The whole repeated mesh in world space, so a click on any copy
                // -- not just the original the child sits at -- selects the
                // pattern. The child meshes are still collected below, so the
                // original copy also reaches the child that draws it.
                let world = Arc::new(apply(&parent, &subtree.mesh));
                out.meshes.insert(id, world);
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
                // The colour rides on the mesh as a per-triangle tag, so a
                // repaint has to miss the cache the way a resize does.
                crate::scene::colour_tag(scene.effective_colour(id)).hash(&mut hasher.0);
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
            Body::Pattern { params } => {
                "pattern".hash(&mut hasher.0);
                hash_params(hasher, params);
                for &child in &node.children {
                    if scene.node(child).visible {
                        self.hash_subtree(scene, child, hasher);
                    }
                }
                node.children.iter().filter(|c| scene.node(**c).visible).count().hash(&mut hasher.0);
            }
            // Neither the recipe a split carries nor the tiling that made its
            // pieces is geometry -- nothing evaluates either until the pieces
            // are joined back together -- so both stay out of the key, and two
            // splits holding the same pieces share one result.
            Body::Split { .. } => {
                "split".hash(&mut hasher.0);
                for &child in &node.children {
                    if scene.node(child).visible {
                        self.hash_subtree(scene, child, hasher);
                    }
                }
                node.children.iter().filter(|c| scene.node(**c).visible).count().hash(&mut hasher.0);
            }
            Body::Mesh { mesh } => {
                "mesh".hash(&mut hasher.0);
                // The geometry itself never changes -- a stored mesh is
                // immutable, and an edit to one replaces it -- so its identity
                // is the allocation it lives in plus how big it is. Hashing a
                // hundred thousand vertices on every keystroke would cost more
                // than rebuilding the shapes the cache exists to avoid.
                (Arc::as_ptr(mesh) as usize).hash(&mut hasher.0);
                mesh.triangle_count().hash(&mut hasher.0);
                crate::scene::colour_tag(scene.effective_colour(id)).hash(&mut hasher.0);
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
/// The mesh a set of nodes evaluates to, in world space, ready to export.
///
/// `Evaluated::node_meshes` holds *primitives* only: a group's result lives in
/// the subtree cache instead, so merging the per-node meshes of a selected
/// boolean group writes its operands as separate overlapping solids rather than
/// the shape the viewport shows. Each node's own subtree is evaluated here
/// instead -- booleans and all -- placed by the frame its parent gave it, and
/// the results are unioned exactly as the scene root would union them.
///
/// `frames` is `Evaluated::node_frames`, which already carries any anchor shift
/// an ancestor applied. A node missing from it is one the last evaluation never
/// reached, and is skipped rather than placed wrongly at the origin.
pub fn selection_mesh(scene: &Scene, ids: &[NodeId], frames: &BTreeMap<NodeId, Xform>) -> Mesh {
    let mut parts: Vec<Mesh> = part_meshes(scene, ids, frames).into_iter().map(|part| part.mesh).collect();
    match parts.len() {
        0 => Mesh::new(),
        1 => parts.pop().unwrap(),
        _ => evaluate_boolean(GroupOp::Union.to_geom(), &parts),
    }
}

/// One evaluated body per node, rather than the single union [`selection_mesh`]
/// makes of them -- what an export that writes each object as its own component
/// needs (issue 58).
///
/// The bodies are the ones the nodes stand for: a boolean group is the shape it
/// evaluates to, not its operands, exactly as in [`selection_mesh`]. Nodes that
/// are hidden, missing, or evaluate to nothing are left out, so the caller never
/// has to write an empty object.
pub struct Part {
    pub id: NodeId,
    pub name: String,
    pub mesh: Mesh,
}

pub fn part_meshes(scene: &Scene, ids: &[NodeId], frames: &BTreeMap<NodeId, Xform>) -> Vec<Part> {
    let mut evaluator = Evaluator::new();
    let cancel = Cancel::new();
    let mut parts = Vec::new();
    for &id in ids {
        if !scene.contains(id) || !scene.node(id).visible {
            continue;
        }
        let Some(parent) = frames.get(&id) else { continue };
        let mesh = apply(parent, &evaluator.subtree(scene, id, &cancel).mesh);
        if mesh.triangle_count() > 0 {
            parts.push(Part { id, name: scene.node(id).name.clone(), mesh });
        }
    }
    parts
}

/// The bodies a user-chosen export writes: one per [`Part`], each already
/// unioned into a single solid (issue 58).
///
/// The marks are read one node at a time, and every one of them is local:
///
/// * no mark, or a mark this walk cannot honour, is a body of that node alone
///   -- so a project nobody has marked comes out exactly as `part_meshes`
///   would, and adding a shape later needs no decision made about it;
/// * [`ExportBody::Split`] on a separable group is not a body at all: its
///   children are considered in its place, which is how an export reaches
///   inside a group;
/// * [`ExportBody::Shared`] merges, into one solid, every node carrying the
///   same number, wherever in the tree they are.
///
/// Bodies come out in the order the tree reaches them, and a merged one is
/// named for the shapes in it, because that name is all a slicer will show.
pub fn body_meshes(scene: &Scene, ids: &[NodeId], frames: &BTreeMap<NodeId, Xform>) -> Vec<Part> {
    let mut roots: Vec<(NodeId, Option<u32>)> = Vec::new();
    for &id in ids {
        collect_bodies(scene, id, &mut roots);
    }

    // Grouped by number, in the order each number is first met, with every
    // unnumbered node a body of its own.
    let mut bodies: Vec<(Option<u32>, Vec<NodeId>)> = Vec::new();
    for (id, key) in roots {
        match key.and_then(|k| bodies.iter_mut().find(|(other, _)| *other == Some(k))) {
            Some((_, members)) => members.push(id),
            None => bodies.push((key, vec![id])),
        }
    }

    bodies
        .into_iter()
        .filter_map(|(_, members)| {
            let mesh = selection_mesh(scene, &members, frames);
            if mesh.triangle_count() == 0 {
                return None;
            }
            let first = *members.first()?;
            Some(Part { id: first, name: body_name(scene, &members), mesh })
        })
        .collect()
}

fn collect_bodies(scene: &Scene, id: NodeId, out: &mut Vec<(NodeId, Option<u32>)>) {
    if !scene.contains(id) || !scene.node(id).visible {
        return;
    }
    match scene.node(id).export_body {
        // Only where the group really can be taken apart. A mark that says
        // otherwise is one the tree has changed under -- a group turned into a
        // difference since it was made -- and the honest reading of it is the
        // one this export can carry out.
        Some(ExportBody::Split) if scene.can_split_for_export(id) => {
            for &child in &scene.node(id).children {
                collect_bodies(scene, child, out);
            }
        }
        Some(ExportBody::Shared(key)) => out.push((id, Some(key))),
        _ => out.push((id, None)),
    }
}

/// What a body is called in the file. One shape lends its own name; several
/// are named for what is in them, because "Body 2" tells a slicer nothing.
fn body_name(scene: &Scene, members: &[NodeId]) -> String {
    let names: Vec<&str> =
        members.iter().filter(|id| scene.contains(**id)).map(|id| scene.node(*id).name.as_str()).collect();
    match names.len() {
        0 => String::new(),
        1 => names[0].to_string(),
        _ => {
            let joined = names.join(" + ");
            // Long enough to name two or three shapes, short enough to read in
            // a slicer's object list.
            if joined.chars().count() <= 64 {
                joined
            } else {
                format!("{} + {} more", names[0], names.len() - 1)
            }
        }
    }
}

pub fn subtree_bounds(scene: &Scene, id: NodeId) -> Option<(Vec3, Vec3)> {
    Evaluator::new().subtree(scene, id, &Cancel::new()).mesh.bounds()
}

/// The geometry a node evaluates to, in the node's *own* frame: its booleans
/// done and its pattern copies laid down, but before its anchor, scale,
/// rotation and position (issue 80).
///
/// The node's own frame, and not the parent's, is what "convert this to a mesh"
/// has to capture. A stored mesh replaces the *body*, and the node keeps its
/// transform -- so capturing the placed geometry and then placing it again would
/// move the shape twice at the moment it was converted, which is the one thing
/// a conversion must never do.
///
/// The anchor is taken back out for the same reason: the node keeps its anchor,
/// and the offset a base anchor applies is recomputed from the stored mesh --
/// which is this same geometry, so it comes out the same. Leaving it in would
/// apply it twice.
pub fn baked_mesh(scene: &Scene, id: NodeId) -> Mesh {
    baked_mesh_in_place(scene, id, Xform::IDENTITY).0
}

/// [`baked_mesh`], and the transform that puts what it returns back where the
/// node stands in the world (issue 82).
///
/// A tool works in the node's own frame -- that is the frame the geometry comes
/// back in, and the frame a cell size in millimetres means something in -- but
/// drawing the tool's work *over the model* needs the way back out again, and
/// the way back is the node's own transform with the anchor shift put on top.
/// It is the same composition [`Evaluator::walk`] records the node's world
/// placement with, which is why the preview lands exactly on the surface rather
/// than an anchor's distance off it.
///
/// `parent` is the node's parent frame in world space --
/// [`Evaluated::node_frames`] -- and [`Xform::IDENTITY`] asks only for the
/// node's own placement within its parent, which is what a caller wanting the
/// mesh alone passes.
///
/// The two come back together because the anchor shift costs a subtree
/// evaluation to work out, and asking twice would pay for it twice.
pub fn baked_mesh_in_place(scene: &Scene, id: NodeId, parent: Xform) -> (Mesh, Xform) {
    if !scene.contains(id) {
        return (Mesh::new(), parent);
    }
    let mut evaluator = Evaluator::new();
    let result = evaluator.subtree(scene, id, &Cancel::new());
    let node = scene.node(id);
    let own = Xform::from_pos_rot_scale(node.position, node.rotation, crate::scene::Node::sane_scale(node.scale));
    // The offset is already in the subtree's mesh, and is zero for every anchor
    // but a base one -- so one number answers both directions: the mesh has it
    // taken back off, and the placement puts it back on.
    let placement = parent.compose(&own).compose(&Xform::from_translation(result.anchor_offset));
    let mesh = apply(&own.inverse(), &result.mesh);
    let mesh = if result.anchor_offset == Vec3::ZERO { mesh } else { mesh.translated(-result.anchor_offset) };
    (mesh, placement)
}

fn combine(
    op: GroupOp,
    children: &[Mesh],
    id: NodeId,
    name: &str,
    errors: &mut Vec<NodeError>,
    cancel: &Cancel,
) -> Mesh {
    if children.is_empty() {
        return Mesh::new();
    }
    // The boolean is where an evaluation's time goes, and it used to be the one
    // part of it that could not be interrupted: the flag was checked where the
    // work is not. A union of dozens of finely tessellated solids ran for
    // minutes with nothing able to stop it, so `EvalWorker::submit`'s
    // `cancel.cancel()` never landed, every newer edit queued behind it, and the
    // application could not be got out of it -- the footer still said
    // "Evaluating..." with every node deleted.
    let result = evaluate_boolean_until(op.to_geom(), children, &|| cancel.is_cancelled());
    if cancel.is_cancelled() {
        // Not a result: an abandoned boolean is an empty or half-built mesh.
        // Returned as it is rather than reported as non-manifold, which it
        // usually is -- and `subtree` keeps it out of the cache, because the
        // run is dropped but the cache would not be.
        return result;
    }
    let mut result = result;
    if op == GroupOp::Hull {
        // A hull is a new surface stretched over the operands, not a selection
        // of their faces: there is no body a given face came from. It takes the
        // first operand's colour, which is the one the group's own colour lands
        // on when a whole group is painted.
        let first = children.first().map_or(0, |mesh| mesh.tag(0));
        result.set_tag(first);
    }
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
    group_meshes: BTreeMap<NodeId, Arc<Mesh>>,
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
    Mesh {
        positions: mesh.positions.iter().map(|&p| xf.point(p)).collect(),
        indices: mesh.indices.clone(),
        tags: mesh.tags.clone(),
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
    fn a_boolean_in_flight_can_be_cancelled() {
        // The flag used to be checked where the work is not: `combine` took no
        // cancellation at all, so once a union started, the flag the interface
        // sets on the next edit could never land. The job ran to the end, every
        // newer edit queued behind it, and with a big enough union the
        // application could not be got out of it -- the footer still said
        // "Evaluating..." with every node deleted.
        //
        // A union of finely tessellated spheres that all overlap is the easiest
        // way to reach a boolean that costs seconds. Cancelled a moment in, it
        // must stop in a moment rather than run to the end.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        for i in 0..4 {
            let id = scene.add_primitive("sphere", group, i).expect("the sphere is in the registry");
            let node = scene.get_mut(id).unwrap();
            node.segments = Some(96);
            node.position = Vec3::new(i as f64 * 12.0, 0.0, 0.0);
        }

        // How long it takes when nobody interrupts it, so the assertion below is
        // measured against this machine rather than against a guess.
        let uninterrupted = std::time::Instant::now();
        let whole = Evaluator::new().evaluate(&scene, &Cancel::new());
        let uninterrupted = uninterrupted.elapsed();
        assert!(!whole.cancelled);
        assert!(
            uninterrupted > std::time::Duration::from_millis(250),
            "this test needs a union that takes a while; it took {uninterrupted:?}"
        );

        let cancel = Cancel::new();
        let flag = cancel.clone();
        let started = std::time::Instant::now();
        let runner = std::thread::spawn(move || Evaluator::new().evaluate(&scene, &cancel));
        std::thread::sleep(std::time::Duration::from_millis(60));
        flag.cancel();
        let out = runner.join().expect("the evaluation thread did not panic");
        let took = started.elapsed();

        assert!(out.cancelled, "the run does not report itself cancelled");
        assert!(took < uninterrupted / 2, "it took {took:?} of the {uninterrupted:?} it takes uninterrupted");
    }

    /// Every colour a mesh's faces are painted, with how many faces each has.
    fn painted(mesh: &Mesh) -> std::collections::BTreeMap<Option<[u8; 3]>, usize> {
        let mut counts = std::collections::BTreeMap::new();
        for i in 0..mesh.indices.len() {
            *counts.entry(crate::scene::Colour::from_tag(mesh.tag(i)).map(|c| c.0)).or_insert(0) += 1;
        }
        counts
    }

    #[test]
    fn a_colour_follows_each_surface_through_a_difference() {
        // What the whole per-face tag exists for: after a painted cutter drills
        // a painted plate, the wall of the hole is the cutter's colour and the
        // plate around it is still the plate's.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        let base = plate(&mut scene, group);
        let hole = cylinder(&mut scene, group, 6.0, 20.0);
        scene.paint_subtree(base, Some(crate::scene::Colour([0x10, 0x20, 0x30])));
        scene.paint_subtree(hole, Some(crate::scene::Colour([0xF0, 0xE0, 0xD0])));

        let result = Evaluator::new().evaluate(&scene, &Cancel::new());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let counts = painted(&result.mesh);
        assert!(counts.contains_key(&Some([0x10, 0x20, 0x30])), "the plate lost its colour: {counts:?}");
        assert!(counts.contains_key(&Some([0xF0, 0xE0, 0xD0])), "the hole wall lost the cutter's: {counts:?}");
        assert!(!counts.contains_key(&None), "some surface came out unpainted: {counts:?}");
    }

    #[test]
    fn painting_a_group_paints_everything_in_it_and_a_shape_can_still_differ() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let a = plate(&mut scene, group);
        let b = cylinder(&mut scene, group, 6.0, 4.0);
        scene.get_mut(b).unwrap().position = Vec3::new(30.0, 0.0, 0.0);

        scene.paint_subtree(group, Some(crate::scene::Colour([1, 2, 3])));
        assert_eq!(scene.effective_colour(a).map(|c| c.0), Some([1, 2, 3]));
        assert_eq!(scene.effective_colour(b).map(|c| c.0), Some([1, 2, 3]));
        // Inherited, not copied: only the group carries a colour of its own.
        assert!(scene.node(a).colour.is_none());

        scene.paint_subtree(b, Some(crate::scene::Colour([9, 9, 9])));
        let counts = painted(&Evaluator::new().evaluate(&scene, &Cancel::new()).mesh);
        assert!(counts.contains_key(&Some([1, 2, 3])), "{counts:?}");
        assert!(counts.contains_key(&Some([9, 9, 9])), "{counts:?}");

        // And painting the group again takes the whole group back, including
        // the shape that had been painted on its own.
        scene.paint_subtree(group, Some(crate::scene::Colour([4, 5, 6])));
        assert_eq!(scene.effective_colour(b).map(|c| c.0), Some([4, 5, 6]));
    }

    #[test]
    fn repainting_re_evaluates_but_moving_a_painted_shape_does_not_recolour_it() {
        // The colour is part of the geometry cache key, or a repaint would show
        // the cached mesh in the old colour.
        let mut scene = Scene::new();
        let root = scene.root();
        let id = plate(&mut scene, root);
        let mut evaluator = Evaluator::new();
        let before = painted(&evaluator.evaluate(&scene, &Cancel::new()).mesh);
        assert_eq!(before.keys().collect::<Vec<_>>(), vec![&None]);

        scene.paint_subtree(id, Some(crate::scene::Colour([7, 7, 7])));
        let after = painted(&evaluator.evaluate(&scene, &Cancel::new()).mesh);
        assert_eq!(after.keys().collect::<Vec<_>>(), vec![&Some([7, 7, 7])]);
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
        let out = combine(GroupOp::Union, &[sliver.clone(), sliver], 42, "Bad group", &mut errors, &Cancel::new());
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
    fn a_pattern_repeats_its_child_without_touching_the_original() {
        // Issue 67: a linear pattern of a 20mm box, three copies stepping 50mm
        // along X, spans one box either end of three centres.
        let mut scene = Scene::new();
        let root = scene.root();
        let pat = scene.add_pattern(root, 0);
        let child = scene.add_primitive("box", pat, 0).unwrap();
        let one_box = {
            let mut solo = Scene::new();
            let r = solo.root();
            solo.add_primitive("box", r, 0).unwrap();
            Evaluator::new().evaluate(&solo, &Cancel::new()).mesh.triangle_count()
        };
        {
            let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
            p.insert("kind".into(), ParamValue::Choice(0));
            p.insert("count".into(), ParamValue::Count(3));
            p.insert("step_x".into(), ParamValue::Length(50.0));
        }
        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (lo, hi) = out.mesh.bounds().unwrap();
        assert!((lo.x + 10.0).abs() < 1e-6, "the first copy is not the original: {lo:?}");
        assert!((hi.x - 110.0).abs() < 1e-6, "the third copy is not 100mm along: {hi:?}");
        assert_eq!(out.mesh.triangle_count(), one_box * 3, "a pattern of three should be three copies");
        let _ = child;

        // The whole repeated mesh is pickable under the pattern's own id, so a
        // click on any copy reaches the pattern.
        assert!(out.node_meshes.contains_key(&pat), "the pattern has no pickable mesh");
    }

    #[test]
    fn a_pattern_whose_copies_touch_is_still_one_solid() {
        // Regression, issue 67: the stock step was 20mm and the stock box is
        // 20mm wide, so the very first thing the pattern tool produced was three
        // copies face to face. Concatenated, that welds into an edge shared by
        // four triangles and the enclosing group reported "Union produced
        // non-manifold geometry: edge (13,12) used 2 times, expected 1" -- the
        // whole scene red on the feature's first use. Unioning the copies gives
        // what the manual duplicates a pattern replaces would have given.
        let mut scene = Scene::new();
        let root = scene.root();
        let pat = scene.add_pattern(root, 0);
        scene.add_primitive("box", pat, 0).unwrap();
        {
            let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
            p.insert("kind".into(), ParamValue::Choice(0));
            p.insert("count".into(), ParamValue::Count(3));
            p.insert("step_x".into(), ParamValue::Length(20.0)); // exactly the box's width
        }
        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        assert!(out.errors.is_empty(), "a touching pattern reported {:?}", out.errors);
        assert!(out.mesh.manifold_issue().is_none(), "the touching copies did not fuse into one solid");
        // Fused end to end, so it spans three boxes and no interior walls are left.
        let (lo, hi) = out.mesh.bounds().unwrap();
        assert!((hi.x - lo.x - 60.0).abs() < 1e-6, "{:?}", hi - lo);
    }

    #[test]
    fn a_pattern_of_copies_that_stand_clear_stays_a_concatenation() {
        // The other half of the bargain: copies with a gap between them must not
        // pay for the boolean kernel, so the result is still exactly the copies.
        let mut scene = Scene::new();
        let root = scene.root();
        let pat = scene.add_pattern(root, 0);
        scene.add_primitive("box", pat, 0).unwrap();
        let one_box = {
            let mut solo = Scene::new();
            let r = solo.root();
            solo.add_primitive("box", r, 0).unwrap();
            Evaluator::new().evaluate(&solo, &Cancel::new()).mesh.triangle_count()
        };
        {
            let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
            p.insert("count".into(), ParamValue::Count(4));
            p.insert("step_x".into(), ParamValue::Length(50.0));
        }
        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        assert!(out.errors.is_empty());
        assert_eq!(out.mesh.triangle_count(), one_box * 4, "the disjoint copies went through the kernel");
    }

    #[test]
    fn a_grid_pattern_cannot_be_asked_for_more_copies_than_anything_can_draw() {
        // Issue 67: each count clamps to 512 on its own, but a grid multiplies
        // three of them -- 512^3 is 134 million transforms, ~14 GB, reachable by
        // typing three numbers. The cap is what stands between that and an
        // out-of-memory kill.
        let mut scene = Scene::new();
        let root = scene.root();
        let pat = scene.add_pattern(root, 0);
        scene.add_primitive("box", pat, 0).unwrap();
        {
            let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
            p.insert("kind".into(), ParamValue::Choice(1));
            for key in ["grid_x", "grid_y", "grid_z"] {
                p.insert(key.into(), ParamValue::Count(512));
            }
            // Spread out, so the cap is what limits the work and not the kernel.
            for key in ["grid_step_x", "grid_step_y", "grid_step_z"] {
                p.insert(key.into(), ParamValue::Length(40.0));
            }
        }
        let params = scene.node(pat).params().unwrap().clone();
        let (wanted, made) = crate::pattern::instance_count(&params);
        assert_eq!(wanted, 512 * 512 * 512, "the per-axis clamp still multiplies out");
        assert_eq!(made, crate::pattern::MAX_INSTANCES);
        assert_eq!(crate::pattern::instances(&params).len(), crate::pattern::MAX_INSTANCES);
        // And it really evaluates, rather than taking the machine down with it.
        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        assert!(out.mesh.triangle_count() > 0);
    }

    #[test]
    fn a_mirror_pattern_reflects_its_child_and_stays_watertight() {
        // A box off to +X, mirrored across the X-normal plane: a matching box at
        // -X, and each copy is still a solid despite the reflection's winding.
        let mut scene = Scene::new();
        let root = scene.root();
        let pat = scene.add_pattern(root, 0);
        let child = scene.add_primitive("box", pat, 0).unwrap();
        scene.get_mut(child).unwrap().position = Vec3::new(30.0, 0.0, 0.0);
        {
            let p = scene.get_mut(pat).unwrap().params_mut().unwrap();
            p.insert("kind".into(), ParamValue::Choice(3)); // Mirror
            p.insert("mirror_axis".into(), ParamValue::Choice(0));
        }
        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (lo, hi) = out.mesh.bounds().unwrap();
        // Symmetric about the origin: the reflection reached -X.
        assert!((lo.x + hi.x).abs() < 1e-6, "the mirror is not symmetric: {lo:?} {hi:?}");
        // Each of the two copies is a closed solid; a reflection whose winding was
        // not flipped would be inside out.
        assert!(out.mesh.manifold_issue().is_none(), "the reflected copy was left inside out");
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

    // -- the bodies issue 80 added ------------------------------------------

    #[test]
    fn baking_a_node_captures_what_it_evaluates_to_without_moving_it() {
        // Issue 80's whole correctness condition: converting must not shift the
        // shape, whatever the node's transform and anchor happen to be.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        let base = plate(&mut scene, group);
        let hole = cylinder(&mut scene, group, 6.0, 40.0);
        scene.get_mut(hole).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
        {
            let node = scene.get_mut(group).unwrap();
            node.position = Vec3::new(30.0, -12.0, 4.0);
            node.rotation = Vec3::new(0.0, 0.0, 35.0);
            node.scale = Vec3::new(1.5, 1.0, 2.0);
            node.anchor = Anchor::Base;
        }
        let before = Evaluator::new().evaluate(&scene, &Cancel::new()).mesh.bounds().unwrap();

        let baked = baked_mesh(&scene, group);
        assert!(baked.triangle_count() > 0);
        let mut converted = scene.clone();
        assert!(converted.convert_to_mesh(group, crate::mesh_data::MeshData::new(baked)));
        assert!(converted.node(group).is_mesh());
        assert!(converted.node(group).children.is_empty(), "the operands outlived the body they made");

        let after = Evaluator::new().evaluate(&converted, &Cancel::new()).mesh.bounds().unwrap();
        assert!((before.0 - after.0).length() < 1e-6, "the shape moved: {before:?} -> {after:?}");
        assert!((before.1 - after.1).length() < 1e-6, "the shape moved: {before:?} -> {after:?}");
        let _ = base;
    }

    #[test]
    fn the_baked_placement_puts_the_geometry_back_exactly_where_the_node_stands() {
        // What a tool's preview is drawn through (issue 82). The tool works in
        // the node's own frame; if the way back out is off by an anchor, a
        // scale or an ancestor, the cells are drawn floating beside the shape
        // they are cutting rather than on it -- and the split still comes out
        // right, so nothing but the eye would catch it.
        //
        // Every one of those is turned on at once, and the answer is checked
        // against the evaluation's own world mesh rather than against a repeat
        // of the arithmetic.
        let mut scene = Scene::new();
        let root = scene.root();
        let outer = scene.add_group(GroupOp::Union, root, 0);
        {
            let node = scene.get_mut(outer).unwrap();
            node.position = Vec3::new(-14.0, 6.0, 3.0);
            node.rotation = Vec3::new(0.0, 25.0, 0.0);
        }
        let group = scene.add_group(GroupOp::Difference, outer, 0);
        plate(&mut scene, group);
        let hole = cylinder(&mut scene, group, 6.0, 40.0);
        scene.get_mut(hole).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
        {
            let node = scene.get_mut(group).unwrap();
            node.position = Vec3::new(30.0, -12.0, 4.0);
            node.rotation = Vec3::new(0.0, 0.0, 35.0);
            node.scale = Vec3::new(1.5, 1.0, 2.0);
            node.anchor = Anchor::Base;
        }

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let parent = out.node_frames[&group];
        let (baked, placement) = baked_mesh_in_place(&scene, group, parent);
        assert!(baked.triangle_count() > 0);

        // Every baked point, put back, must land on the world mesh the
        // evaluation drew -- so the two boxes agree to the last micron.
        let placed = bounds_of(baked.positions.iter().map(|&p| placement.point(p))).expect("the baked mesh has points");
        let world = bounds_of(out.mesh.positions.iter().copied()).expect("the scene has points");
        assert!((placed.0 - world.0).length() < 1e-6, "the placement is off: {placed:?} against {world:?}");
        assert!((placed.1 - world.1).length() < 1e-6, "the placement is off: {placed:?} against {world:?}");

        // And the mesh itself is unchanged by asking for the placement with it.
        let alone = baked_mesh(&scene, group);
        assert_eq!(alone.positions, baked.positions, "asking for the placement changed the geometry");
    }

    #[test]
    fn a_centre_anchored_node_is_placed_without_an_anchor_shift() {
        // The other half of the same sum: an offset applied where there is none
        // to apply would push the preview off the shape by half its height.
        let mut scene = Scene::new();
        let root = scene.root();
        let id = plate(&mut scene, root);
        scene.get_mut(id).unwrap().position = Vec3::new(3.0, 4.0, 5.0);
        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (_, placement) = baked_mesh_in_place(&scene, id, out.node_frames[&id]);
        assert!((placement.point(Vec3::ZERO) - Vec3::new(3.0, 4.0, 5.0)).length() < 1e-9);
    }

    #[test]
    fn a_stored_mesh_is_placed_by_its_node_like_any_other_body() {
        let mut scene = Scene::new();
        let root = scene.root();
        let id = scene.add_mesh(
            "Baked",
            crate::mesh_data::MeshData::new(simple3d_geom::primitives::box_mesh(20.0, 20.0, 20.0)),
            root,
            0,
        );
        scene.get_mut(id).unwrap().position = Vec3::new(50.0, 0.0, 0.0);
        scene.get_mut(id).unwrap().scale = Vec3::new(2.0, 1.0, 1.0);

        let out = Evaluator::new().evaluate(&scene, &Cancel::new());
        let (lo, hi) = out.mesh.bounds().unwrap();
        assert!((hi.x - lo.x - 40.0).abs() < 1e-6, "the scale was not applied: {}", hi.x - lo.x);
        assert!(((lo.x + hi.x) * 0.5 - 50.0).abs() < 1e-6, "the position was not applied");
        assert!(out.node_meshes.contains_key(&id), "a mesh body has nothing to pick");
    }

    #[test]
    fn an_interrupted_evaluation_leaves_nothing_of_itself_in_the_cache() {
        // The bug this holds back, seen in the running application: three boxes
        // in a union group, all of them visible, and a viewport with nothing in
        // it -- "4 nodes, 0 triangles" in 0.03 ms, which is the time a cache hit
        // takes and not the time a boolean takes.
        //
        // An abandoned boolean returns an empty mesh (`evaluate_boolean_until`
        // gives back `Mesh::new()` the moment `give_up` says so), and the
        // worker's `Evaluator` -- and so its subtree cache -- lives for as long
        // as the application does. Cached under the content hash of a perfectly
        // good scene, that empty mesh was what every later evaluation of the
        // same content got back: the shapes vanished the moment an edit landed
        // while a boolean was running, and stayed gone until something changed
        // the hash again.
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        // Heavy enough that the boolean is still running when the cancel lands.
        for i in 0..2 {
            let id = scene.add_primitive("sphere", group, i).unwrap();
            scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 12.0, 0.0, 0.0);
            scene.get_mut(id).unwrap().segments = Some(96);
            scene.get_mut(id).unwrap().params_mut().unwrap().insert("diameter".into(), ParamValue::Length(30.0));
        }
        let whole = Evaluator::new().evaluate(&scene, &Cancel::new()).mesh.triangle_count();
        assert!(whole > 0, "the scene this is about evaluates to nothing even uninterrupted");

        // One evaluator across both runs, exactly as the worker keeps one.
        let mut evaluator = Evaluator::new();
        let cancel = Arc::new(Cancel::new());
        let flag = Arc::clone(&cancel);
        // Interrupted after the run has started and while the boolean is in it,
        // which is where a newer edit interrupts one. Landing late is harmless:
        // the run then finishes honestly and the assertion below still holds.
        let hand = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(5));
            flag.cancel();
        });
        let interrupted = evaluator.evaluate(&scene, &cancel);
        hand.join().unwrap();
        assert!(interrupted.cancelled, "the run was not interrupted, so this proves nothing");

        // The next frame: the same scene, nothing cancelled, the same evaluator.
        let again = evaluator.evaluate(&scene, &Cancel::new());
        assert_eq!(
            again.mesh.triangle_count(),
            whole,
            "the abandoned run left its empty mesh in the cache: the shapes are gone from every frame after it"
        );
    }
}

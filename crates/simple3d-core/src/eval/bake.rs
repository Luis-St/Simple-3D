//! Flattening evaluated nodes into one mesh, in place or in world space.

use super::*;
use crate::scene::{GroupOp, NodeId, Scene};
use crate::xform::Xform;
use simple3d_geom::{evaluate_boolean, evaluate_boolean_traced, BooleanOp, Mesh, Vec3};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The mesh a set of nodes evaluates to, in world space, ready to export.
///
/// Each node's subtree is evaluated here, booleans and all, since `Evaluated::node_meshes` holds
/// primitives only and merging them wrote boolean operands as overlapping solids. `frames` is
/// `Evaluated::node_frames`; nodes missing from it are skipped rather than placed at the origin.
pub fn selection_mesh(scene: &Scene, ids: &[NodeId], frames: &BTreeMap<NodeId, Xform>) -> Mesh {
    let mut parts: Vec<Mesh> = part_meshes(scene, ids, frames).into_iter().map(|part| part.mesh).collect();
    match parts.len() {
        0 => Mesh::new(),
        1 => parts.pop().unwrap(),
        _ => evaluate_boolean(BooleanOp::Union, &parts),
    }
}

pub fn subtree_bounds(scene: &Scene, id: NodeId) -> Option<(Vec3, Vec3)> {
    Evaluator::new().subtree(scene, id, &Cancel::new()).mesh.bounds()
}

/// The geometry a node evaluates to in its own frame, before anchor, scale, rotation and position
/// (issue 80). The node keeps its transform and anchor after conversion, so including either
/// would apply them twice.
pub fn baked_mesh(scene: &Scene, id: NodeId) -> Mesh {
    baked_mesh_in_place(scene, id, Xform::IDENTITY).0
}

/// [`baked_mesh`], plus the transform placing it where the node stands in the world (issue 82).
///
/// The same composition [`Evaluator::walk`] uses, so previews land on the surface. `parent` is the
/// node's parent frame ([`Evaluated::node_frames`]), or [`Xform::IDENTITY`] for the placement
/// within its parent. Returned together since the anchor shift costs a subtree evaluation.
pub fn baked_mesh_in_place(scene: &Scene, id: NodeId, parent: Xform) -> (Mesh, Xform) {
    if !scene.contains(id) {
        return (Mesh::new(), parent);
    }
    let mut evaluator = Evaluator::new();
    let result = evaluator.subtree(scene, id, &Cancel::new());
    let node = scene.node(id);
    let own = Xform::from_pos_rot_scale(node.position, node.rotation, crate::scene::Node::sane_scale(node.scale));
    // The offset is in the subtree's mesh (zero except for base anchors): taken off the mesh, put
    // back on the placement.
    let placement = parent.compose(&own).compose(&Xform::from_translation(result.anchor_offset));
    let mesh = apply(&own.inverse(), &result.mesh);
    let mesh = if result.anchor_offset == Vec3::ZERO { mesh } else { mesh.translated(-result.anchor_offset) };
    (mesh, placement)
}

/// The mesh a group's operation makes of its children, with the untouched children and where
/// their vertices start ([`simple3d_geom::Traced`]).
pub(crate) fn combine(
    op: GroupOp,
    children: &[Mesh],
    id: NodeId,
    name: &str,
    errors: &mut Vec<NodeError>,
    cancel: &Cancel,
) -> simple3d_geom::Traced {
    if children.is_empty() {
        return (Mesh::new(), Vec::new());
    }
    // The boolean must be cancellable: it used to ignore the flag, so a long union could not be
    // stopped and every later edit queued behind it.
    let Some(boolean) = op.to_geom() else {
        return side_by_side(children);
    };
    let (mut result, untouched) = evaluate_boolean_traced(boolean, children, &|| cancel.is_cancelled());
    if cancel.is_cancelled() {
        // Not a result: returned as is rather than reported non-manifold, and `subtree` keeps it out
        // of the cache.
        return (result, Vec::new());
    }
    if op == GroupOp::Hull {
        // A hull is a new surface with no source body per face; it takes the first operand's colour,
        // where a painted group's colour lands.
        let first = children.first().map_or(0, |mesh| mesh.tag(0));
        result.set_tag(first);
    }
    if let Some(issue) = result.manifold_issue() {
        errors.push(NodeError {
            node: id,
            name: name.to_string(),
            message: format!("{} produced non-manifold geometry: {issue}", op.label()),
        });
        // Fall back to the operands side by side so the scene still previews; the node is flagged and
        // export refuses while the error stands (spec section 5.2).
        return side_by_side(children);
    }
    (result, untouched)
}

/// The children appended one after another, all untouched.
fn side_by_side(children: &[Mesh]) -> simple3d_geom::Traced {
    let mut out = Mesh::new();
    let mut untouched = Vec::with_capacity(children.len());
    for (index, child) in children.iter().enumerate() {
        untouched.push((index, out.positions.len() as u32));
        out.append(child);
    }
    (out, untouched)
}

#[derive(Default)]
pub(crate) struct Collected {
    pub(super) meshes: BTreeMap<NodeId, Arc<Mesh>>,
    pub(super) group_meshes: BTreeMap<NodeId, Arc<Mesh>>,
    pub(super) frames: BTreeMap<NodeId, Xform>,
    pub(super) local_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    pub(super) world_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    pub(super) placements: BTreeMap<NodeId, Xform>,
}

pub(crate) use simple3d_geom::aabb::bounds_of;

pub(crate) fn apply(xf: &Xform, mesh: &Mesh) -> Mesh {
    Mesh {
        positions: mesh.positions.iter().map(|&p| xf.point(p)).collect(),
        indices: mesh.indices.clone(),
        tags: mesh.tags.clone(),
    }
}

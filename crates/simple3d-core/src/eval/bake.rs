//! Flattening evaluated nodes into one mesh, in place or in world space.

use super::*;
use crate::scene::{GroupOp, NodeId, Scene};
use crate::xform::Xform;
use simple3d_geom::{evaluate_boolean, evaluate_boolean_until, Mesh, Vec3};
use std::collections::BTreeMap;
use std::sync::Arc;

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

pub(crate) fn combine(
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
pub(crate) struct Collected {
    pub(super) meshes: BTreeMap<NodeId, Arc<Mesh>>,
    pub(super) group_meshes: BTreeMap<NodeId, Arc<Mesh>>,
    pub(super) frames: BTreeMap<NodeId, Xform>,
    pub(super) local_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    pub(super) world_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
}

pub(crate) fn bounds_of(points: impl Iterator<Item = Vec3>) -> Option<(Vec3, Vec3)> {
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for p in points {
        bounds = Some(match bounds {
            Some((lo, hi)) => (lo.min(p), hi.max(p)),
            None => (p, p),
        });
    }
    bounds
}

pub(crate) fn apply(xf: &Xform, mesh: &Mesh) -> Mesh {
    Mesh {
        positions: mesh.positions.iter().map(|&p| xf.point(p)).collect(),
        indices: mesh.indices.clone(),
        tags: mesh.tags.clone(),
    }
}

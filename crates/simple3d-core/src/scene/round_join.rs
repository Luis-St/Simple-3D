//! Joining roundings already made at an inside corner between them (issue 88).
//!
//! Two edges of an L rounded in separate steps each end flat at the inside corner, held by their own
//! objects, and leave a spike of the corner standing between them. The corner can still be picked
//! later: the roundings whose edges meet there are taken off their objects and made one edit on the
//! union holding them, with the corner mitred, as if edges and corner had been picked together.

use super::*;
use crate::xform::Xform;
use simple3d_geom::rounding::FeatureEdge;

/// An inside corner between treated edges of roundings already in the scene.
#[derive(Clone, Debug, PartialEq)]
pub struct RoundJoin {
    /// The corner, in world space.
    pub at: Vec3,
    /// The two edges meeting there, in world space, for telling an inside corner from an outside one.
    pub edges: [FeatureEdge; 2],
    /// The round edits holding those edges, as (node, index into its edits), in scene order.
    pub parts: Vec<(NodeId, usize)>,
}

impl Scene {
    /// Every point where two outside edges of the scene's roundings, of one kind and size, meet at an
    /// angle on a shared face without being mitred there. Whether the solid turns inward between them
    /// is left to the caller, which has the model to look at.
    pub fn round_joins(&self, frames: &BTreeMap<NodeId, Xform>) -> Vec<RoundJoin> {
        let mut edits: Vec<((NodeId, usize), RoundEdit)> = Vec::new();
        for (&id, node) in &self.nodes {
            for (index, edit) in node.edits.iter().enumerate() {
                let (ObjectEdit::Round(edit), Some(frame)) = (edit, self.own_frame(id, frames)) else { continue };
                edits.push(((id, index), edit.carried(&frame)));
            }
        }
        let near = |p: Vec3, q: Vec3| (p - q).length() < 1e-6;
        let alike = |p: &RoundEdit, q: &RoundEdit| {
            p.kind == q.kind && p.segments == q.segments && (p.size - q.size).abs() < 1e-6
        };
        let edges: Vec<(usize, &FeatureEdge)> =
            edits.iter().enumerate().flat_map(|(i, (_, e))| e.edges.iter().map(move |edge| (i, edge))).collect();
        let mut out: Vec<RoundJoin> = Vec::new();
        for (n, &(i, p)) in edges.iter().enumerate() {
            for &(j, q) in &edges[n + 1..] {
                let Some(at) = [p.a, p.b].into_iter().find(|&a| near(a, q.a) || near(a, q.b)) else { continue };
                let mitred = edits[i].1.joints.iter().chain(&edits[j].1.joints).any(|&k| near(k, at));
                let framing = p.convex
                    && q.convex
                    && p.direction().dot(q.direction()).abs() < 1.0 - 1e-6
                    && p.normals.iter().any(|m| q.normals.iter().any(|n| n.dot(*m) > 1.0 - 1e-6));
                if mitred || !framing || !alike(&edits[i].1, &edits[j].1) || out.iter().any(|o| near(o.at, at)) {
                    continue;
                }
                let mut parts = vec![edits[i].0, edits[j].0];
                parts.dedup();
                out.push(RoundJoin { at, edges: [*p, *q], parts });
            }
        }
        out
    }

    /// The one edit the roundings `parts` become, mitred at each of the corners `at` between them
    /// (world space), and the node holding it: the union holding all of them, or `None` when that is
    /// an assembly or a split.
    pub fn joined_round_edit(
        &self,
        parts: &[(NodeId, usize)],
        at: &[Vec3],
        frames: &BTreeMap<NodeId, Xform>,
    ) -> Option<(NodeId, RoundEdit)> {
        let mut world: Option<RoundEdit> = None;
        for &(id, index) in parts {
            let ObjectEdit::Round(edit) = self.nodes.get(&id)?.edits.get(index)? else { return None };
            let edit = edit.carried(&self.own_frame(id, frames)?);
            match world.as_mut() {
                None => world = Some(edit),
                Some(world) => {
                    world.edges.extend(edit.edges);
                    world.corners.extend(edit.corners);
                    world.joints.extend(edit.joints);
                }
            }
        }
        let mut world = world?;
        world.joints.extend(at);
        let (first, _) = *parts.first()?;
        let holder = if parts.iter().all(|&(id, _)| id == first) {
            first
        } else {
            self.shared_union(parts.iter().map(|&(id, _)| id))?
        };
        Some((holder, world.carried(&self.own_frame(holder, frames)?.inverse())))
    }
}

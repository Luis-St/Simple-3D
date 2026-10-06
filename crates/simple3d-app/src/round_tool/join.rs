//! Picking the inside corner between roundings made in earlier steps (issue 88): the roundings meeting
//! there are taken off their objects by the draft and stand as one edit, mitred at the corner, on the
//! union holding them ([`simple3d_core::scene::RoundJoin`]).

use super::*;
use simple3d_core::scene::{RoundEdit, RoundJoin};
use simple3d_geom::Vec3;

/// A round edit on a node, by its index among the node's edits.
type EditAt = (NodeId, usize);

impl App {
    /// The corners between the scene's roundings where the solid turns inward, so a rounding of
    /// each edge stops flat there, judged on `base`, the model without the draft.
    pub(crate) fn round_join_corners(&self, base: &Mesh, treatment: Treatment) -> Vec<Vec3> {
        let inside = |join: &RoundJoin| {
            join.edges.iter().all(|edge| {
                let side = if pick::near(edge.a, join.at) { 0 } else { 1 };
                !simple3d_geom::rounding::run_on(base, edge, treatment)[side]
            })
        };
        let joins = self.scene.round_joins(&self.evaluated.node_frames);
        joins.into_iter().filter(inside).map(|join| join.at).collect()
    }

    /// The edits the picked corners between earlier roundings make, and the edits they replace, which
    /// are taken off: per node from the last, so each index still names its edit when it goes.
    pub(super) fn round_joined_edits(&self, joints: &[Vec3]) -> (Vec<(NodeId, RoundEdit)>, Vec<EditAt>) {
        let frames = &self.evaluated.node_frames;
        // Joins sharing a rounding become one edit, mitred at each of their corners.
        let mut groups: Vec<(Vec<EditAt>, Vec<Vec3>)> = Vec::new();
        let picked = self.scene.round_joins(frames).into_iter().filter(|j| joints.iter().any(|&p| pick::near(p, j.at)));
        for join in picked {
            let (mut parts, mut at) = (join.parts, vec![join.at]);
            groups.retain(|(other, corners)| {
                let shared = other.iter().any(|p| parts.contains(p));
                if shared {
                    parts.extend(other.iter().filter(|p| !parts.contains(p)).copied().collect::<Vec<_>>());
                    at.extend(corners);
                }
                !shared
            });
            groups.push((parts, at));
        }
        let mut made = Vec::new();
        let mut taken = Vec::new();
        for (parts, at) in groups {
            if let Some(edit) = self.scene.joined_round_edit(&parts, &at, frames) {
                made.push(edit);
                taken.extend(parts);
            }
        }
        taken.sort_by(|a, b| b.cmp(a));
        (made, taken)
    }
}

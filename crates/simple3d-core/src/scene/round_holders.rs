//! Which node holds the rounding of each picked edge and corner (issue 88).
//!
//! On its own, a pick goes to the node a push/pull of its face would ([`Scene::edit_holder`]). Picks
//! of different objects joined by a picked corner, such as the two edges of an inside corner where
//! two boxes join, go together to the union holding those objects: only there is the corner one
//! shape, so their rounds can meet and cut into a third object filling the corner. Edges merely
//! touching, with their corner not picked, stay with their own objects and end flat. An edge merged
//! across objects, given once for each, goes to the union holding them too.

use super::*;
use simple3d_geom::rounding::{Corner, FeatureEdge};

impl Scene {
    /// The holder of each of `edges` and then each of `corners`, by the objects they were found on.
    pub(super) fn round_holders(
        &self,
        edges: &[(NodeId, FeatureEdge)],
        corners: &[(NodeId, Corner)],
        joints: &[Vec3],
    ) -> Vec<NodeId> {
        let points: Vec<Vec<Vec3>> =
            edges.iter().map(|(_, e)| vec![e.a, e.b]).chain(corners.iter().map(|(_, c)| vec![c.at])).collect();
        let owners: Vec<NodeId> = edges.iter().map(|(o, _)| *o).chain(corners.iter().map(|(o, _)| *o)).collect();
        // Picks meeting at a picked corner, inside or outside, are one cluster.
        let picked: Vec<Vec3> = joints.iter().copied().chain(corners.iter().map(|(_, c)| c.at)).collect();
        let mut cluster: Vec<usize> = (0..points.len()).collect();
        fn find(cluster: &mut [usize], i: usize) -> usize {
            let mut i = i;
            while cluster[i] != i {
                cluster[i] = cluster[cluster[i]];
                i = cluster[i];
            }
            i
        }
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                let near = |p: &Vec3, q: &Vec3| (*p - *q).length() < 1e-6;
                // One edge merged across objects comes once per object it runs over.
                let same = points[i].len() == 2 && points[i].iter().all(|p| points[j].iter().any(|q| near(p, q)));
                let meet = same
                    || points[i]
                        .iter()
                        .any(|p| points[j].iter().any(|q| near(p, q)) && picked.iter().any(|c| near(p, c)));
                if meet {
                    let (a, b) = (find(&mut cluster, i), find(&mut cluster, j));
                    cluster[a] = b;
                }
            }
        }
        let own: Vec<NodeId> = owners.iter().map(|&owner| self.edit_holder(owner)).collect();
        let mut holders = own.clone();
        for root in 0..points.len() {
            let members: Vec<usize> = (0..points.len()).filter(|&i| find(&mut cluster, i) == root).collect();
            let Some(&first) = members.first() else { continue };
            if members.iter().all(|&i| own[i] == own[first]) {
                continue;
            }
            if let Some(shared) = self.shared_union(members.iter().map(|&i| own[i])) {
                for i in members {
                    holders[i] = shared;
                }
            }
        }
        holders
    }

    /// The nearest union, or the scene, holding every one of `ids`; `None` when that is an assembly
    /// or a split, whose parts are never one shape.
    pub(super) fn shared_union(&self, ids: impl Iterator<Item = NodeId> + Clone) -> Option<NodeId> {
        let first = ids.clone().next()?;
        let mut at = self.nodes[&first].parent?;
        while !ids.clone().all(|id| self.is_ancestor_of(at, id)) {
            at = self.nodes[&at].parent?;
        }
        let union = at == self.root || self.nodes[&at].group_op() == Some(GroupOp::Union);
        union.then_some(at)
    }
}

//! Taking a rounding out of the node holding it as mesh nodes of its own (issue 88).
//!
//! A rounding held by an object or a group goes where push/pull's extracted solids go: its cutters in
//! a difference wrapped around the holder, its fillets beside it. One held by the scene, as a corner
//! joined between objects is, has no node to wrap: each object its cutters reach is wrapped in a
//! difference with a copy of them, so each keeps its own colour where it is cut, and its fillets go
//! beside them in the scene.

use super::*;
use crate::mesh_data::MeshData;
use crate::xform::Xform;
use simple3d_geom::{evaluate_boolean, BooleanOp, Mesh};
use std::collections::BTreeMap;
use std::sync::Arc;

impl Scene {
    /// Take rounding edit `index` out of `id` as mesh nodes of its own: its cutters in a difference
    /// with the object, its fillets beside it, made against `model`, the evaluated model in world
    /// space, with `bounds` each node's world bounds. Returns the new nodes; `None` for an edit that
    /// is not a rounding or made nothing.
    pub fn extract_round_edit(
        &mut self,
        id: NodeId,
        index: usize,
        frames: &BTreeMap<NodeId, Xform>,
        bounds: &BTreeMap<NodeId, (Vec3, Vec3)>,
        model: &Mesh,
    ) -> Option<Vec<NodeId>> {
        let ObjectEdit::Round(edit) = self.nodes.get(&id)?.edits.get(index)? else { return None };
        let world = edit.carried(&self.own_frame(id, frames)?);
        let (adds, cuts) = world.solids(model);
        let rounding = edit.kind == RoundKind::Round;
        let mut made = Vec::new();
        // Added solids first: they stand beside the object, which a cut then wraps.
        for (placing, pieces) in [(Placing::Add, adds), (Placing::Cut, cuts)] {
            let mesh = match pieces.len() {
                0 => continue,
                1 => pieces.into_iter().next()?,
                _ => evaluate_boolean(BooleanOp::Union, &pieces),
            };
            let suffix = match (placing, rounding) {
                (Placing::Cut, true) => "rounding",
                (Placing::Cut, false) => "bevel",
                (Placing::Add, true) => "fillet",
                (Placing::Add, false) => "bevel fill",
            };
            let name = format!("{} {suffix}", self.nodes[&id].name);
            let placed = match id == self.root {
                true => self.place_on_scene(placing, &name, &mesh, frames, bounds),
                false => self.place_mesh(id, placing, &name, &mesh, frames).into_iter().collect(),
            };
            made.extend(placed);
        }
        if made.is_empty() {
            return None;
        }
        self.remove_edit(id, index);
        Some(made)
    }

    /// Add `mesh`, given in world space, as mesh nodes `placing` among the scene's objects: for a cut,
    /// one copy in a difference wrapped around each object whose bounds it reaches; for an addition,
    /// one beside the first of them.
    fn place_on_scene(
        &mut self,
        placing: Placing,
        name: &str,
        mesh: &Mesh,
        frames: &BTreeMap<NodeId, Xform>,
        bounds: &BTreeMap<NodeId, (Vec3, Vec3)>,
    ) -> Vec<NodeId> {
        let Some((lo, hi)) = mesh.bounds() else { return Vec::new() };
        let reached = |id: &NodeId| {
            bounds.get(id).is_some_and(|&(l, h)| {
                l.x <= hi.x && lo.x <= h.x && l.y <= hi.y && lo.y <= h.y && l.z <= hi.z && lo.z <= h.z
            })
        };
        let root = self.root;
        let touched: Vec<NodeId> =
            self.nodes[&root].children.iter().copied().filter(|id| self.nodes[id].visible && reached(id)).collect();
        // The scene's children all sit in the frame the scene gives them.
        let Some(&frame) = touched.first().and_then(|first| frames.get(first)) else { return Vec::new() };
        let inverse = frame.inverse();
        let mut local = mesh.clone();
        for p in local.positions.iter_mut() {
            *p = inverse.point(*p);
        }
        let body = || Body::Mesh { mesh: Arc::new(MeshData { mesh: local.clone() }) };
        let at = |scene: &Scene, id: NodeId| scene.nodes[&root].children.iter().position(|&c| c == id).unwrap_or(0);
        if placing == Placing::Add {
            let first = touched[0];
            let colour = self.effective_colour(first);
            let id = self.insert_fresh(self.unique_name(name), body(), root, at(self, first) + 1);
            self.nodes.get_mut(&id).expect("just made").colour = colour;
            return vec![id];
        }
        let mut made = Vec::new();
        for host in touched {
            let cut_name = self.unique_name(&format!("{} cut", self.nodes[&host].name));
            let cut = self.insert_fresh(cut_name, Body::Group { op: GroupOp::Difference }, root, at(self, host));
            self.unlink(host);
            self.link(host, cut, 0);
            made.push(self.insert_fresh(self.unique_name(name), body(), cut, 1));
        }
        made
    }
}

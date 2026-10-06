//! The picks standing in the document as edits while the window is open, so the model shows them.

use super::*;
use simple3d_core::scene::RoundEdit;
use std::hash::{Hash, Hasher};

impl App {
    /// Bring the draft and the ghost in line with the picks and numbers. Cheap when nothing changed,
    /// so it is called every frame the tool is open.
    pub(crate) fn sync_round_draft(&mut self) {
        self.refresh_round_base();
        let Some(tool) = self.round_tool.as_ref() else { return };
        let key = made_for(tool);
        if tool.made_for == Some(key) {
            return;
        }
        // Merging switched, the edges picked before may no longer be edges of the model.
        if let Some(features) = self.round_features() {
            let tool = self.round_tool.as_mut().expect("checked above");
            tool.edges.retain(|e| features.edges.iter().any(|f| pick::same_edge(e, f)));
        }
        let Some(tool) = self.round_tool.as_ref() else { return };
        let key = made_for(tool);
        // Worked out on the document without the draft, whose indices the joined roundings name.
        let lifted = self.lift_round_draft();
        let Some(tool) = self.round_tool.as_ref() else { return };
        let (edges, corners) = self.round_targets();
        let world = RoundEdit {
            kind: tool.kind,
            size: tool.size().max(0.01),
            segments: tool.segments.clamp(1, 64),
            edges: Vec::new(),
            corners: Vec::new(),
            joints: tool.joints.clone(),
        };
        let mut edits = self.scene.round_edits(&edges, &corners, &world, &self.evaluated.node_frames);
        let (joined, taken) = self.round_joined_edits(&tool.joints);
        edits.extend(joined);
        let ghost = ghost::Ghost::build(tool, &edges, &corners);
        let draft = Lifted {
            added: edits.into_iter().map(|(holder, edit)| (holder, ObjectEdit::Round(edit))).collect(),
            taken: taken
                .into_iter()
                .filter_map(|(id, i)| Some((id, i, self.scene.node(id).edits.get(i)?.clone())))
                .collect(),
        };
        if !(lifted.is_empty() && draft.is_empty()) {
            self.touch();
        }
        self.put_round_draft_back(&draft);
        let Some(tool) = self.round_tool.as_mut() else { return };
        tool.draft = draft.added;
        tool.taken = draft.taken;
        tool.ghost = ghost;
        tool.made_for = Some(key);
    }

    /// Take the draft out of the document, putting back the roundings it took off, and return what
    /// was there to put back with [`App::put_round_draft_back`].
    pub(crate) fn lift_round_draft(&mut self) -> Lifted {
        let Some(tool) = self.round_tool.as_ref() else { return Lifted::default() };
        // Last first, so two edits on one node come off in the order they went on.
        let draft: Vec<(NodeId, ObjectEdit)> = tool.draft.iter().rev().cloned().collect();
        let taken = tool.taken.clone();
        let mut added: Vec<_> =
            draft.into_iter().filter(|(holder, edit)| self.scene.take_edit(*holder, edit)).collect();
        added.reverse();
        // In the reverse of the order they came off, so each goes back where it was.
        let mut taken: Vec<_> = taken
            .into_iter()
            .rev()
            .filter(|(id, index, edit)| self.scene.insert_edit(*id, *index, edit.clone()))
            .collect();
        taken.reverse();
        Lifted { added, taken }
    }

    /// Put a lifted draft back into the document.
    pub(crate) fn put_round_draft_back(&mut self, draft: &Lifted) {
        for (id, index, _) in &draft.taken {
            self.scene.remove_edit(*id, *index);
        }
        for (holder, edit) in &draft.added {
            self.scene.push_edit(*holder, edit.clone());
        }
    }

    /// Close the tool, taking its draft back out of the document.
    pub fn cancel_round_tool(&mut self) {
        if self.lift_round_draft().is_empty() {
            self.round_tool = None;
            return;
        }
        self.round_tool = None;
        self.touch();
    }

    /// The picked edges and the corners to treat, in world space, each with the object it is on.
    pub(crate) fn round_targets(&self) -> Targets {
        let Some(tool) = self.round_tool.as_ref() else { return (Vec::new(), Vec::new()) };
        let owner_of =
            |sources: &[u32]| sources.iter().map(|&s| NodeId::from(s)).find(|&id| id != 0 && self.scene.contains(id));
        let features = self.round_features();
        // An edge merged across objects is on each object it runs over, so their union holds it.
        let edges = tool
            .edges
            .iter()
            .flat_map(|edge| {
                let mut owners: Vec<NodeId> = features
                    .iter()
                    .flat_map(|f| &f.pieces)
                    .filter(|piece| within(piece, edge))
                    .filter_map(|piece| owner_of(&piece.sources))
                    .collect();
                owners.dedup();
                if owners.is_empty() {
                    owners.extend(owner_of(&edge.sources));
                }
                owners.sort_unstable();
                owners.dedup();
                let edge = match tool.extend_edges {
                    true => {
                        simple3d_geom::rounding::extend_edge(&tool.base, edge, tool.treatment().reach(edge.opening()))
                    }
                    false => *edge,
                };
                owners.into_iter().map(move |owner| (owner, edge))
            })
            .collect();
        let corners = corners_to_treat(tool, &features)
            .into_iter()
            .filter_map(|corner| Some((owner_of(&[corner.source])?, corner)))
            .collect();
        (edges, corners)
    }

    /// Follow the model while the tool has no draft in it and the picture is up to date, so a change
    /// made beside the tool is picked on.
    fn refresh_round_base(&mut self) {
        let settled = !self.dirty && !self.worker.is_busy();
        let Some(tool) = self.round_tool.as_mut() else { return };
        if settled && tool.draft.is_empty() && !Arc::ptr_eq(&tool.base, &self.evaluated.mesh) {
            tool.base = self.evaluated.mesh.clone();
        }
    }
}

/// The draft as it stands in the document: the edits it adds, and the roundings it took off to join
/// them at a picked corner, each with where it was.
#[derive(Default)]
pub(crate) struct Lifted {
    pub(crate) added: Vec<(NodeId, ObjectEdit)>,
    pub(crate) taken: Vec<(NodeId, usize, ObjectEdit)>,
}

impl Lifted {
    pub(crate) fn is_empty(&self) -> bool {
        self.added.is_empty() && self.taken.is_empty()
    }
}

/// The edges and corners to treat, each with the object it is on.
pub(crate) type Targets = (Vec<(NodeId, FeatureEdge)>, Vec<(NodeId, Corner)>);

/// Whether `piece` lies on `edge`, between its ends.
fn within(piece: &FeatureEdge, edge: &FeatureEdge) -> bool {
    let t = edge.direction();
    [piece.a, piece.b].iter().all(|&p| {
        let s = (p - edge.a).dot(t);
        (p - edge.a - t * s).length() < 1e-4 && s > -1e-4 && s < edge.length() + 1e-4
    })
}

/// Everything the draft is made from.
fn made_for(tool: &RoundTool) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut point = |p: simple3d_geom::Vec3| [p.x, p.y, p.z].map(f64::to_bits).hash(&mut hasher);
    tool.edges.iter().for_each(|edge| {
        point(edge.a);
        point(edge.b);
    });
    tool.corners.iter().for_each(|corner| point(corner.at));
    tool.joints.iter().for_each(|&joint| point(joint));
    (
        tool.edges.len(),
        tool.corners.len(),
        tool.joints.len(),
        tool.kind,
        tool.segments,
        tool.blend_corners,
        tool.merge_edges,
        tool.extend_edges,
    )
        .hash(&mut hasher);
    tool.size().to_bits().hash(&mut hasher);
    (Arc::as_ptr(&tool.base) as usize).hash(&mut hasher);
    hasher.finish()
}

/// The picked corners, and with blending on, every corner of the model whose edges are all picked.
fn corners_to_treat(tool: &RoundTool, features: &Option<Rc<Features>>) -> Vec<Corner> {
    let mut out = tool.corners.clone();
    if !tool.blend_corners {
        return out;
    }
    let Some(features) = features else { return out };
    for corner in &features.corners {
        if out.iter().any(|c| pick::near(c.at, corner.at)) {
            continue;
        }
        let picked = |dir: simple3d_geom::Vec3| {
            tool.edges.iter().any(|e| {
                let from = if pick::near(e.a, corner.at) {
                    e.direction()
                } else if pick::near(e.b, corner.at) {
                    -e.direction()
                } else {
                    return false;
                };
                from.dot(dir) > 1.0 - 1e-6
            })
        };
        if corner.edges.iter().all(|&(dir, _)| picked(dir)) {
            out.push(corner.clone());
        }
    }
    out
}

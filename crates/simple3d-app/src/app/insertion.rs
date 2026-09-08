//! Where a new shape goes: beside what is selected, clear of what is there.

use super::*;
use simple3d_core::config::Placement;
use simple3d_core::scene::{GroupOp, NodeId};
use simple3d_geom::Vec3;

impl App {
    /// The topmost selected nodes: selecting a group and one of its children acts
    /// on the group only.
    pub fn top_level_selection(&self) -> Vec<NodeId> {
        let order = self.scene.depth_first();
        let mut tops: Vec<NodeId> = self
            .selection
            .iter()
            .copied()
            .filter(|id| self.scene.contains(*id))
            .filter(|&id| !self.selection.iter().any(|&other| other != id && self.scene.is_ancestor_of(other, id)))
            .collect();
        tops.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
        tops.dedup();
        tops
    }

    /// What an outliner drag started on `source` actually carries: the whole
    /// selection when the row that was grabbed is part of it, and that row
    /// alone otherwise -- dragging something unselected is a statement about
    /// that node, not about whatever was selected before (issue 43).
    ///
    /// Topmost nodes only, in document order, which is both the order they land
    /// in and the set `Scene::reparent_many` expects.
    pub fn dragged_nodes(&self, source: NodeId) -> Vec<NodeId> {
        if self.is_selected(source) && self.selection.len() > 1 {
            let tops = self.top_level_selection();
            if !tops.is_empty() {
                return tops;
            }
        }
        vec![source]
    }

    pub fn add_node(&mut self, type_id: Option<&str>, op: GroupOp) {
        self.edit("Add", None);
        let (parent, index) = self.scene.insertion_point(self.primary());
        let created = match type_id {
            Some(type_id) => self.scene.add_primitive(type_id, parent, index),
            None => Some(self.scene.add_group(op, parent, index)),
        };
        match created {
            Some(id) => {
                // Where a new shape lands is the user's choice; the palette's
                // hint line says which choice is in force. How big the shape is
                // is part of the answer, so the point is asked for only now, with
                // the node in the scene and its own size there to be measured.
                let at = self.insertion_point_world(self.near_face_x(&[id]).unwrap_or(0.0));
                if let Some(node) = self.scene.get_mut(id) {
                    node.position = at;
                }
                // A pattern that has just gained its first child can now be
                // measured, the same as when one is dropped in or pasted in.
                // Without this an empty pattern kept the stock 20 mm step, and
                // a 20 mm shape added into it afterwards was repeated at
                // exactly its own width -- one welded bar rather than shapes
                // standing clear, which is not what the tool gives for the same
                // shapes the other way round.
                self.size_fresh_patterns();
                self.select_only(id);
                self.status = Status::Info(format!("Added {}", self.scene.node(id).name));
            }
            None => self.status = Status::Warning("Unknown primitive type".into()),
        }
    }

    /// Where the near side of what is being added sits relative to its own
    /// origin, along X, across all of it.
    ///
    /// Measured from the nodes themselves rather than guessed from their
    /// parameters, because a shape's width is not always one of them: the
    /// regular polyhedra name an edge length, and the capsule drives no X
    /// extent at all. `None` when there is nothing to measure, or when the
    /// placement in force does not care -- only "beside the selection" does, so
    /// nothing is built for the other three.
    pub(super) fn near_face_x(&self, ids: &[NodeId]) -> Option<f64> {
        if self.settings.placement != Placement::BesideSelection {
            return None;
        }
        ids.iter()
            .filter_map(|&id| simple3d_core::eval::subtree_bounds(&self.scene, id))
            .map(|(lo, _)| lo.x)
            .reduce(f64::min)
    }

    /// Where the next shape goes, in world millimetres, under the current
    /// placement choice.
    ///
    /// `near_face_x` is where the near side of the thing being added sits
    /// relative to its own origin -- for a centred 40 mm box, -20. Only
    /// "beside the selection" needs it, and it needs it badly: the point it
    /// answers with is written to `Node::position`, so leaving the shape's own
    /// width out of the sum buries it half inside what it was meant to stand
    /// clear of. Pass 0 where nothing is being added and the answer is only
    /// being described, as the palette's hint line does.
    ///
    /// World rather than parent-frame: adding into a rotated group would
    /// otherwise put the shape somewhere else entirely. `Node::position` is in
    /// the parent's coordinates, so the answer is carried back through the
    /// parent's frame before it is written.
    pub fn insertion_point_world(&self, near_face_x: f64) -> Vec3 {
        let world = match self.settings.placement {
            Placement::Origin => Vec3::ZERO,
            Placement::Cursor => self.cursor.unwrap_or(Vec3::ZERO),
            Placement::ViewCentre => {
                let step = self.move_snap();
                let t = self.scene.camera.target;
                Vec3::new((t.x / step).round() * step, (t.y / step).round() * step, (t.z / step).round() * step)
            }
            Placement::BesideSelection => match self.selection_bounds() {
                // Clear of the selection along +X with one step of air, so the
                // new shape is next to what is selected rather than inside it.
                Some((lo, hi)) => {
                    Vec3::new(hi.x + self.move_snap() - near_face_x, (lo.y + hi.y) * 0.5, (lo.z + hi.z) * 0.5)
                }
                None => Vec3::ZERO,
            },
        };
        let (parent, _) = self.scene.insertion_point(self.primary());
        match self.evaluated.node_frames.get(&parent) {
            // The frame stored for a node is its *parent's*; a child of `parent`
            // is placed in `parent`'s own frame, which is that composed with its
            // transform.
            Some(frame) => {
                let node = self.scene.node(parent);
                frame
                    .compose(&simple3d_core::xform::Xform::from_pos_rot(node.position, node.rotation))
                    .inverse()
                    .point(world)
            }
            None => world,
        }
    }
}

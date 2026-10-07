//! Where a new shape goes: beside the selection, clear of what is there.

use super::*;
use simple3d_core::config::Placement;
use simple3d_core::scene::{GroupOp, NodeId};
use simple3d_geom::Vec3;

impl App {
    /// The topmost selected nodes: a selected group and its child act on the group only.
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

    /// What an outliner drag from `source` carries: the selection if `source` is in it, else just that
    /// row (issue 43). Topmost nodes in document order, as `Scene::reparent_many` expects.
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
                // Asked only now, with the node in the scene, since its size is part of the answer.
                let at = self.insertion_point_in(parent, self.near_face(&[id]).unwrap_or(0.0));
                if let Some(node) = self.scene.get_mut(id) {
                    node.position = at;
                }
                // A pattern's first child lets it be sized, as when dropped or pasted in.
                self.size_fresh_patterns();
                self.select_only(id);
                self.status = Status::Info(hints::added(self, id));
                self.start_placing(id);
            }
            None => self.status = Status::Warning("Unknown primitive type".into()),
        }
    }

    /// The side of the selection a shape added beside it goes on: the face of the selection's box
    /// that looks most towards the camera, so it lands on the face being looked at.
    pub fn beside_side(&self) -> (usize, f64) {
        // From the target towards the eye, as `View` builds it.
        let camera = self.scene.camera;
        let (yaw, pitch) = (camera.yaw.to_radians(), camera.pitch.to_radians());
        let towards = Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin());
        let axis = (0..3).max_by(|&a, &b| towards.get(a).abs().total_cmp(&towards.get(b).abs())).unwrap_or(0);
        (axis, if towards.get(axis) < 0.0 { -1.0 } else { 1.0 })
    }

    /// The near side of what is being added along [`App::beside_side`], relative to its origin,
    /// measured from the nodes since not every shape has a width parameter. `None` if nothing to
    /// measure or not needed (only "beside the selection" needs it).
    pub(super) fn near_face(&self, ids: &[NodeId]) -> Option<f64> {
        if self.settings.placement != Placement::BesideSelection {
            return None;
        }
        let (axis, sign) = self.beside_side();
        let sides = ids.iter().filter_map(|&id| simple3d_core::eval::subtree_bounds(&self.scene, id));
        if sign > 0.0 {
            sides.map(|(lo, _)| lo.get(axis)).reduce(f64::min)
        } else {
            sides.map(|(_, hi)| hi.get(axis)).reduce(f64::max)
        }
    }

    /// Where the next shape goes in world millimetres under the current placement. `near_face` keeps
    /// the shape's own size from burying it in the selection; pass 0 when only describing.
    pub fn insertion_point_world(&self, near_face: f64) -> Vec3 {
        match self.settings.placement {
            Placement::Origin => Vec3::ZERO,
            // Put down where it is first, until the pointer carries it onto a surface.
            Placement::Cursor | Placement::OnSurface => self.cursor.unwrap_or(Vec3::ZERO),
            Placement::ViewCentre => {
                let step = self.move_snap();
                let t = self.scene.camera.target;
                Vec3::new((t.x / step).round() * step, (t.y / step).round() * step, (t.z / step).round() * step)
            }
            Placement::BesideSelection => match self.selection_bounds() {
                // Touching the selection's face towards the camera, centred on it otherwise.
                Some((lo, hi)) => {
                    let (axis, sign) = self.beside_side();
                    let mut at = (lo + hi) * 0.5;
                    let side = if sign > 0.0 { hi.get(axis) } else { lo.get(axis) };
                    at.set(axis, side - near_face);
                    at
                }
                None => Vec3::ZERO,
            },
        }
    }

    /// [`App::insertion_point_world`] as a `Node::position` under `parent`, so a shape added into a
    /// moved or rotated group still lands there. Only for writing: shown, it read as the group's offset.
    pub fn insertion_point_in(&self, parent: NodeId, near_face: f64) -> Vec3 {
        let world = self.insertion_point_world(near_face);
        match self.evaluated.node_frames.get(&parent) {
            // The stored frame is the parent's; a child of `parent` lives in that composed with its transform.
            Some(frame) => {
                let node = self.scene.node(parent);
                let scale = simple3d_core::scene::Node::sane_scale(node.scale);
                frame
                    .compose(&simple3d_core::xform::Xform::from_pos_rot_scale(node.position, node.rotation, scale))
                    .inverse()
                    .point(world)
            }
            None => world,
        }
    }
}

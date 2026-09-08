//! The features of a body a drag can snap onto, kept between frames.

use super::*;
use crate::gizmo::{self};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// Everything one body offers a snap or a measurement: the notable points on it,
/// and the lines the principal planes leave across its surface -- the marks the
/// renderer draws on the solid, which are as catchable as any other line in the
/// picture.
#[derive(Default)]
pub struct BodySnaps {
    pub features: Vec<crate::snap::Feature>,
    pub marks: Vec<(Vec3, Vec3)>,
}

/// One body's snap targets, shared out of the cache without copying them.
pub(crate) type Snaps = std::rc::Rc<BodySnaps>;

/// What the cache holds per node: which mesh the targets were found on --
/// identified by the address of its `Arc`, which changes on re-evaluation and
/// nowhere else -- together with what the settings were showing, since the axis
/// crossings (issue 78) and the plane marks are part of the answer, and the
/// targets themselves.
pub(crate) type CachedSnaps = ((usize, u8), Snaps);

impl App {
    /// The dragged node and everything under it: the bodies a drag is carrying,
    /// which geometry snapping must never snap to.
    pub(super) fn drag_subtree(&self, id: NodeId) -> Vec<NodeId> {
        std::iter::once(id).chain(self.scene.descendants(id)).collect()
    }

    /// Every feature of the body a drag is carrying, as an offset from that
    /// node's origin. Taken once, when the handle is grabbed.
    ///
    /// Which feature should meet the target used to be decided here too, by
    /// looking for one within the catch radius of the cursor. A drag always
    /// starts on a manipulator handle, and those sit a fixed 78 screen pixels out
    /// along an axis -- never on the body's own geometry except by coincidence --
    /// so the answer was almost always "none", and it was the node's *origin*
    /// that landed on the target. Two boxes snapped together interpenetrated by
    /// half, which is not what "snap this corner to that corner" means.
    ///
    /// They are kept as *offsets*, and gathered before the body has moved,
    /// because `Evaluated` lags a drag: the meshes still describe where the body
    /// was at the last evaluation while `Node::position` is already live. An
    /// offset from the origin is the same either way, being a fact about the
    /// shape rather than about where it currently sits.
    pub(super) fn drag_feature_offsets(&self, id: NodeId) -> Vec<Vec3> {
        let Some(frame) = self.evaluated.node_frames.get(&id) else { return Vec::new() };
        let world_origin = frame.point(self.scene.node(id).position);
        let mut offsets = Vec::new();
        for n in self.drag_subtree(id) {
            let Some(mesh) = self.evaluated.node_meshes.get(&n) else { continue };
            offsets.extend(self.snaps_of(n, mesh).features.iter().map(|f| f.point - world_origin));
        }
        offsets
    }

    /// Snap a resize so the face being pulled lands on the nearest feature of
    /// another body under the pointer (issue 68). Returns the world point it
    /// caught, or `None` when nothing was in reach, in which case the grid
    /// resize stands.
    ///
    /// A resize is a drag, and it snapped only to the grid step. Pulling a plate
    /// out until it meets the block beside it is the same gesture as sliding it
    /// there, and it wants the same answer. Only a *face* handle: a corner moves
    /// three faces at once, and there is no one face to put on a point.
    pub(super) fn apply_resize_snap(
        &mut self,
        id: NodeId,
        view: &crate::view::View,
        cursor: egui::Pos2,
        mods: gizmo::Mods,
    ) -> Option<(Vec3, f64)> {
        let exclude = self.drag_subtree(id);
        let (target, _) = self.nearest_feature_excluding(view, cursor, &exclude)?;
        // Taken out and put back so the drag can write the scene the app owns.
        let mut drag = self.drag.take()?;
        let applied = drag.resize_face_to(&mut self.scene, target.point, mods.symmetric);
        self.drag = Some(drag);
        applied.map(|extent| (target.point, extent))
    }
}

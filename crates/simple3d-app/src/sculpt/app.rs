//! A stroke as the application runs it: started on a node, committed as one undo step, cancelled,
//! and settled once the evaluation shows the result.

use super::*;
use crate::app::App;
use simple3d_core::scene::{Anchor, Node};

impl App {
    /// From `id`'s own mesh coordinates to the world, as the evaluation places `mesh` there: the
    /// parent frame, the node's placement, and a base-anchored shape lifted onto its base.
    pub(crate) fn mesh_placement(&self, id: NodeId, mesh: &MeshData) -> Option<Xform> {
        let node = self.scene.get(id)?;
        let placement = Xform::from_pos_rot_scale(node.position, node.rotation, Node::sane_scale(node.scale));
        let own = self.evaluated.node_frames.get(&id)?.compose(&placement);
        let lift = match node.anchor {
            Anchor::Base => Vec3::new(0.0, 0.0, -mesh.mesh.bounds()?.0.z),
            Anchor::Centre => Vec3::ZERO,
        };
        Some(own.compose(&Xform::from_translation(lift)))
    }

    /// Start a stroke on mesh node `id`, keeping the session when it still matches the node.
    /// `false` when `id` is no mesh or has not been evaluated yet.
    pub(crate) fn begin_stroke(&mut self, id: NodeId) -> bool {
        let Some(mesh) = self.scene.get(id).and_then(|node| node.mesh()).cloned() else { return false };
        let Some(placement) = self.mesh_placement(id, &mesh) else { return false };
        let colour = self.scene.effective_colour(id).map(|colour| colour.tag());
        let current = self.sculpt.as_ref().is_some_and(|sculpt| {
            sculpt.target == id
                && Arc::ptr_eq(&sculpt.base, &mesh)
                && sculpt.placement == placement
                && sculpt.colour == colour
        });
        if !current {
            self.sculpt = Some(Sculpt::new(id, mesh, placement, colour));
        }
        let lagging = !self.scene_renderable.parts.contains_key(&id);
        let sculpt = self.sculpt.as_mut().expect("made above");
        sculpt.lagging = lagging;
        sculpt.begin();
        self.invalidate_image();
        true
    }

    /// End the stroke, writing the copy into the document as one undo step labelled `label`. A stroke
    /// that moved nothing leaves no step, and one whose mesh changed underneath it is dropped.
    pub(crate) fn end_stroke(&mut self, label: &str) {
        let Some(sculpt) = self.sculpt.as_mut() else { return };
        let Some(stroke) = sculpt.stroke.take() else { return };
        let held = self.scene.get(sculpt.target).and_then(|node| node.mesh());
        if stroke.is_empty() || !held.is_some_and(|mesh| Arc::ptr_eq(mesh, &sculpt.base)) {
            sculpt.stroke = Some(stroke);
            self.cancel_stroke();
            return;
        }
        let result = Arc::new(sculpt.result());
        // Welding the result may merge vertices a stroke pressed together; the copy then no longer
        // numbers them as the node does, and the next stroke starts afresh.
        let same = result.mesh.positions.len() == sculpt.renderable.mesh.positions.len();
        sculpt.base = result.clone();
        sculpt.settling = true;
        let target = sculpt.target;
        if !same {
            self.sculpt = None;
        }
        self.edit(label, None);
        self.scene.set_mesh(target, result);
        self.invalidate_image();
    }

    /// Abandon the stroke, putting the copy back as it was before it.
    pub(crate) fn cancel_stroke(&mut self) {
        let Some(sculpt) = self.sculpt.as_mut() else { return };
        if sculpt.stroke.is_some() {
            sculpt.restore();
            self.invalidate_image();
        }
    }

    /// Stop drawing a written-back copy once the evaluation has caught up with it.
    pub(crate) fn settle_stroke(&mut self) {
        let caught_up = !self.dirty && !self.worker.is_busy();
        if let Some(sculpt) = self.sculpt.as_mut().filter(|sculpt| sculpt.settling && caught_up) {
            sculpt.settling = false;
            self.invalidate_image();
        }
    }

    /// The working copy, while it is drawn in place of its node.
    pub(crate) fn sculpt_shown(&self) -> Option<&Sculpt> {
        self.sculpt.as_ref().filter(|sculpt| sculpt.shown() && self.scene.contains(sculpt.target))
    }
}

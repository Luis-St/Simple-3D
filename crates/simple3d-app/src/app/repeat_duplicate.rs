//! Duplicating with the last move repeated (issue 121): duplicate, move or turn the copy, then duplicate
//! it with the repeat and the next copy goes as far on again, so a row or ring is built without the
//! pattern tool. From the keyboard the chord previews while held and copies when let go.

use super::*;
use simple3d_core::scene::NodeId;
use simple3d_core::unit::wrap_degrees;
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;

impl App {
    /// Where the copy of `id` goes when the move is repeated: as far on from `id`, in place and turn, as
    /// `id` is from the node it was duplicated from. Nothing when `id` is not a copy the last duplicate
    /// made, or its original has gone or moved to another group.
    pub(super) fn repeated_step(&self, id: NodeId) -> Option<(Vec3, Vec3)> {
        let &(source, _) = self.last_duplicate.iter().find(|&&(_, copy)| copy == id)?;
        let (source, copy) = (self.scene.get(source)?, self.scene.get(id)?);
        if source.parent != copy.parent {
            return None;
        }
        let before = Xform::from_pos_rot(source.position, source.rotation);
        let after = Xform::from_pos_rot(copy.position, copy.rotation);
        // The step taking the original to the copy, taken once more from the copy.
        let next = after.compose(&before.inverse()).compose(&after);
        let rotation = if source.rotation == copy.rotation {
            copy.rotation
        } else {
            let turned = Xform::rotation_of_axes(next.axis(0), next.axis(1), next.axis(2));
            let tidy = |degrees: f64| wrap_degrees((degrees * 1e9).round() / 1e9);
            Vec3::new(tidy(turned.x), tidy(turned.y), tidy(turned.z))
        };
        Some((next.t, rotation))
    }

    /// While the chord is held, each selected copy with a move to repeat and what carries its body to
    /// where its copy would go, for the renderer to draw as templates.
    pub(crate) fn repeat_templates(&self) -> Vec<(NodeId, Xform)> {
        if !self.repeat_hold {
            return Vec::new();
        }
        self.top_level_selection()
            .into_iter()
            .filter_map(|id| {
                let (position, rotation) = self.repeated_step(id)?;
                let node = self.scene.get(id)?;
                let scale = simple3d_core::scene::Node::sane_scale(node.scale);
                let parent = self.evaluated.node_frames.get(&id).copied().unwrap_or(Xform::IDENTITY);
                let now = parent.compose(&Xform::from_pos_rot_scale(node.position, node.rotation, scale));
                let then = parent.compose(&Xform::from_pos_rot_scale(position, rotation, scale));
                Some((id, then.compose(&now.inverse())))
            })
            .collect()
    }

    /// The chord was pressed: preview until it is let go.
    pub(crate) fn start_repeat_hold(&mut self) {
        if self.selection.is_empty() {
            self.status = Status::Info("Nothing to duplicate".into());
            return;
        }
        self.repeat_hold = true;
        self.invalidate_image();
    }

    /// Follow the held chord: once any of its keys is let go, the copies are made.
    pub(crate) fn follow_repeat_hold(&mut self, held: bool) {
        if self.repeat_hold && !held {
            self.duplicate(true);
            self.invalidate_image();
        }
    }

    /// Drop the preview without copying, when the keyboard is taken away mid-hold.
    pub(crate) fn cancel_repeat_hold(&mut self) {
        if std::mem::take(&mut self.repeat_hold) {
            self.invalidate_image();
        }
    }
}

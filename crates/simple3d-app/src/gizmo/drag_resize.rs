//! Resizing: a face pulled to a place, written back as a dimension.

use super::*;
use simple3d_core::primitive::{AxisDriver, ParamValue};
use simple3d_core::scene::{Node, Scene};
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;

impl Drag {
    pub(super) fn drivable(&self, axis: usize) -> Option<AxisDriver> {
        self.gizmo.drivers[axis]
    }

    /// Whether this axis can be sized at all under the current mode. Resize
    /// needs a parameter to write; scale needs nothing.
    pub(super) fn sizeable(&self, axis: usize) -> bool {
        self.gizmo.mode == Mode::Scale || self.drivable(axis).is_some()
    }

    /// The node's extent along one of its own axes **in world millimetres** at
    /// the moment the drag began -- the local extent times every scale between
    /// it and the world.
    ///
    /// World, because that is what a drag measures. The conversion back into a
    /// dimension, or into a factor, happens in one place, in `size_axis`.
    pub(super) fn start_extent(&self, axis: usize) -> f64 {
        let local = get_axis(self.start_local_hi, axis) - get_axis(self.start_local_lo, axis);
        local * self.gizmo.axis_scale[axis]
    }
}

impl Drag {
    /// Grow the given axis by `outward` world millimetres on the given side, and
    /// return the extent actually achieved.
    ///
    /// The two sizing modes part company only in what they write. **Resize**
    /// solves the driver's `extent = value * factor` for the value that produces
    /// the extent asked for -- that is the whole of "resize writes dimensions",
    /// and no scale factor is involved. **Scale** multiplies the node's own
    /// factor by however much bigger the extent got, which needs nothing of the
    /// shape underneath and so works on a group too.
    ///
    /// Both then shift the node by half the change, in the direction of the face
    /// being dragged, so the opposite face stays exactly where it was.
    /// Resize so the face this drag is pulling lands on `target` (issue 68).
    ///
    /// The same one-dimensional answer a face handle already gives, with the
    /// cursor's place along the axis replaced by the snap target's: the face
    /// moves outward by however far the target is from where the face began, and
    /// nothing else about the body changes. Returns the extent it reached, or
    /// `None` when this drag has no single face to place -- a corner moves three
    /// at once, and there is no one face to put on a point.
    pub fn resize_face_to(&mut self, scene: &mut Scene, target: Vec3, symmetric: bool) -> Option<f64> {
        let Handle::ResizeFace(axis, positive) = self.handle else { return None };
        let anchor = self.gizmo.own.point(self.gizmo.face_centre(axis, positive));
        let outward = (target - anchor).dot(self.gizmo.axes[axis]) * if positive { 1.0 } else { -1.0 };
        self.size_axis(scene, axis, outward, symmetric, positive)
    }

    pub(super) fn size_axis(
        &self,
        scene: &mut Scene,
        axis: usize,
        outward: f64,
        symmetric: bool,
        positive: bool,
    ) -> Option<f64> {
        let scaling = self.gizmo.mode == Mode::Scale;
        let start_extent = self.start_extent(axis);
        // A dimension cannot go to zero or negative; clamp rather than let the
        // generator produce inverted geometry.
        let target_extent = (start_extent + outward * if symmetric { 2.0 } else { 1.0 }).max(MIN_EXTENT);

        if scaling {
            // Nothing to be a factor *of*: an axis with no extent cannot be
            // scaled into one.
            if start_extent <= MIN_EXTENT {
                return None;
            }
            let factor = target_extent / start_extent;
            let mut scale = self.start_scale;
            let grown = (get_axis(self.start_scale, axis) * factor).max(Node::MIN_SCALE);
            set_axis(&mut scale, axis, grown);
            scene.get_mut(self.node)?.scale = scale;
        } else {
            let driver = self.drivable(axis)?;
            if driver.factor.abs() < 1e-12 {
                return None;
            }
            let local_extent = target_extent / self.gizmo.axis_scale[axis];
            let params = scene.get_mut(self.node)?.params_mut()?;
            params.insert(driver.param.to_string(), ParamValue::Length(local_extent / driver.factor));
        }

        let node = scene.get_mut(self.node)?;
        if symmetric {
            node.position = self.start_position;
        } else {
            // `position` is in the parent's frame, so the shift is measured
            // there: the world change divided by whatever the ancestors scale
            // by, which is everything in `axis_scale` except this node's own.
            let ancestor = (self.gizmo.axis_scale[axis] / get_axis(self.start_scale, axis)).max(1e-9);
            let change = (target_extent - start_extent) / ancestor;
            let mut local_shift = Vec3::ZERO;
            set_axis(&mut local_shift, axis, change / 2.0 * if positive { 1.0 } else { -1.0 });
            let parent_shift = Xform::from_pos_rot(Vec3::ZERO, self.start_rotation).vector(local_shift);
            node.position = self.start_position + parent_shift;
        }
        Some(target_extent)
    }

    /// Write a world-space movement back to `Node::position`, which lives in the
    /// parent's frame.
    pub(super) fn write_position(&self, scene: &mut Scene, world_delta: Vec3) {
        let parent = self.gizmo.parent;
        let target = parent.point(self.start_position) + world_delta;
        if let Some(node) = scene.get_mut(self.node) {
            node.position = parent.inverse().point(target);
        }
    }

    /// Escape during a drag restores the pre-drag values exactly (spec section 6.2).
    pub fn cancel(&self, scene: &mut Scene) {
        if let Some(node) = scene.get_mut(self.node) {
            node.position = self.start_position;
            node.rotation = self.start_rotation;
            node.scale = self.start_scale;
            if let Some(params) = node.params_mut() {
                *params = self.start_params.clone();
            }
        }
    }
}

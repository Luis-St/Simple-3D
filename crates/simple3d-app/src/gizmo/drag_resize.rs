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

    /// Whether this axis can be sized in the current mode: resize needs a parameter, scale nothing.
    pub(super) fn sizeable(&self, axis: usize) -> bool {
        self.gizmo.mode == Mode::Scale || self.drivable(axis).is_some()
    }

    /// The node's extent along an own axis in world millimetres at drag start. Converted back to a
    /// dimension or factor only in `size_axis`.
    pub(super) fn start_extent(&self, axis: usize) -> f64 {
        let local = self.start_local_hi.get(axis) - self.start_local_lo.get(axis);
        local * self.gizmo.axis_scale[axis]
    }
}

impl Drag {
    /// Resize so the pulled face lands on `target` (issue 68): the face moves out by the target's
    /// distance from its start. `None` for a corner, which moves three faces.
    pub fn resize_face_to(&mut self, scene: &mut Scene, target: Vec3, symmetric: bool) -> Option<f64> {
        let Handle::ResizeFace(axis, positive) = self.handle else { return None };
        let anchor = self.gizmo.own.point(self.gizmo.face_centre(axis, positive));
        let outward = (target - anchor).dot(self.gizmo.axes[axis]) * if positive { 1.0 } else { -1.0 };
        self.symmetric = symmetric;
        self.size_axis(scene, axis, outward, symmetric, positive)
    }

    /// Grow `axis` by `outward` world millimetres on one side and return the extent reached. Resize
    /// solves the driver for the value giving that extent; scale multiplies the own factor. Both
    /// shift the node by half the change so the opposite face stays put.
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
        // Clamped so the generator never produces inverted geometry.
        let target_extent = (start_extent + outward * if symmetric { 2.0 } else { 1.0 }).max(MIN_EXTENT);

        if scaling {
            // An axis with no extent cannot be scaled into one.
            if start_extent <= MIN_EXTENT {
                return None;
            }
            let factor = target_extent / start_extent;
            let mut scale = self.start_scale;
            let grown = (self.start_scale.get(axis) * factor).max(Node::MIN_SCALE);
            scale.set(axis, grown);
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
            // `position` is in the parent's frame, so divide the world change by the ancestors' scale alone.
            let ancestor = (self.gizmo.axis_scale[axis] / self.start_scale.get(axis)).max(1e-9);
            let change = (target_extent - start_extent) / ancestor;
            let mut local_shift = Vec3::ZERO;
            local_shift.set(axis, change / 2.0 * if positive { 1.0 } else { -1.0 });
            let parent_shift = Xform::from_pos_rot(Vec3::ZERO, self.start_rotation).vector(local_shift);
            node.position = self.start_position + parent_shift;
        }
        Some(target_extent)
    }

    /// Write a world-space movement back to `Node::position`, which lives in the parent's frame.
    pub(super) fn write_position(&self, scene: &mut Scene, world_delta: Vec3) {
        let parent = self.gizmo.parent;
        let target = parent.point(self.start_position) + world_delta;
        if let Some(node) = scene.get_mut(self.node) {
            node.position = parent.inverse().point(target);
        }
    }

    /// Escape during a drag restores the pre-drag values exactly (spec section 6.2).
    pub fn cancel(&self, scene: &mut Scene) {
        for other in &self.others {
            other.cancel(scene);
        }
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

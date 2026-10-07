//! Resizing: a face pulled to a place, written back as a dimension.

use super::*;
use simple3d_core::config::SizeAnchor;
use simple3d_core::primitive::{AxisDriver, ParamValue};
use simple3d_core::scene::{Node, Scene};
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
    pub(crate) fn start_extent(&self, axis: usize) -> f64 {
        let local = self.start_local_hi.get(axis) - self.start_local_lo.get(axis);
        local * self.gizmo.axis_scale[axis]
    }
}

/// The part of the box that holds still along one axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fixed {
    Lo,
    Hi,
    Centre,
}

impl Fixed {
    /// What the anchor holds while the side on `positive` is pulled; `symmetric` (Ctrl) holds the
    /// centre whatever the anchor says.
    pub(crate) fn of(anchor: SizeAnchor, positive: bool, symmetric: bool) -> Fixed {
        if symmetric {
            return Fixed::Centre;
        }
        match anchor {
            SizeAnchor::Opposite if positive => Fixed::Lo,
            SizeAnchor::Opposite => Fixed::Hi,
            SizeAnchor::Centre => Fixed::Centre,
        }
    }

    /// How much the extent grows per millimetre the pulled side travels: about the centre both
    /// sides go, so twice.
    pub(crate) fn growth(self) -> f64 {
        if self == Fixed::Centre {
            2.0
        } else {
            1.0
        }
    }
}

impl Drag {
    pub(super) fn fixed(&self, positive: bool, symmetric: bool) -> Fixed {
        Fixed::of(self.gizmo.anchor, positive, symmetric)
    }

    /// Resize so the pulled face lands on `target` (issue 68): the face moves out by the target's
    /// distance from its start. `None` for a corner, which moves three faces.
    pub fn resize_face_to(&mut self, scene: &mut Scene, target: Vec3, symmetric: bool) -> Option<f64> {
        let Handle::ResizeFace(axis, positive) = self.handle else { return None };
        let anchor = self.gizmo.own.point(self.gizmo.face_centre(axis, positive));
        let outward = (target - anchor).dot(self.gizmo.axes[axis]) * if positive { 1.0 } else { -1.0 };
        self.symmetric = symmetric;
        self.size_axis(scene, axis, outward, symmetric, positive)
    }

    /// Grow `axis` by `outward` world millimetres, the distance the pulled side travelled, and return
    /// the extent reached. Resize solves the driver for the value giving that extent; scale multiplies
    /// the own factor. Both then shift the node so the side the anchor holds stays put (issue 120).
    pub(crate) fn size_axis(
        &self,
        scene: &mut Scene,
        axis: usize,
        outward: f64,
        symmetric: bool,
        positive: bool,
    ) -> Option<f64> {
        let scaling = self.gizmo.mode == Mode::Scale;
        let start_extent = self.start_extent(axis);
        let fixed = self.fixed(positive, symmetric);
        // Clamped so the generator never produces inverted geometry.
        let target_extent = (start_extent + outward * fixed.growth()).max(MIN_EXTENT);

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

        // The box grows about the node's origin, so the held point drifts from `held` to `held * k`
        // along the axis; the node is shifted back by that much. Covers off-centre origins too.
        let (lo, hi) = (self.start_local_lo.get(axis), self.start_local_hi.get(axis));
        let held = match fixed {
            Fixed::Lo => lo,
            Fixed::Hi => hi,
            Fixed::Centre => (lo + hi) / 2.0,
        };
        let k = if start_extent > MIN_EXTENT { target_extent / start_extent } else { 1.0 };
        let world_shift = self.gizmo.own.axis_vector(axis) * (held * (1.0 - k));
        let node = scene.get_mut(self.node)?;
        node.position = self.start_position + self.gizmo.parent.inverse().vector(world_shift);
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

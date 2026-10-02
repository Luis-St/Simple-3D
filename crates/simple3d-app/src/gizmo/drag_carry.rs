//! The rest of a multiple selection, carried along by the dragged node's change.

use super::*;
use simple3d_core::primitive::ParamsExt;
use simple3d_core::scene::{Node, Scene};
use simple3d_core::unit::wrap_degrees;
use simple3d_core::xform::Xform;

impl Drag {
    /// Give every carried node the change the dragged one has made so far, read from the scene so a
    /// geometry snap is carried too. A move or turn moves them by the dragged node's world change (a
    /// turn orbiting them about the same pivot and adding the angle to theirs); scale gives each the
    /// same factor and resize the same change in millimetres, keeping the same side in place.
    pub fn carry(&self, scene: &mut Scene) {
        if self.others.is_empty() {
            return;
        }
        let Some(node) = scene.get(self.node) else { return };
        let (position, rotation, scale) = (node.position, node.rotation, Node::sane_scale(node.scale));
        let params = node.params().cloned().unwrap_or_default();
        match self.handle {
            Handle::MoveAxis(_) | Handle::MovePlane(_) | Handle::RotateRing(_) => {
                let now = self.gizmo.parent.compose(&Xform::from_pos_rot_scale(position, rotation, scale));
                let change = now.compose(&self.gizmo.own.inverse());
                let turn = match self.handle {
                    Handle::RotateRing(axis) => Some((axis, rotation.get(axis) - self.start_rotation.get(axis))),
                    _ => None,
                };
                for other in &self.others {
                    let parent = other.gizmo.parent;
                    let target = change.point(parent.point(other.start_position));
                    let Some(carried) = scene.get_mut(other.node) else { continue };
                    carried.position = parent.inverse().point(target);
                    if let Some((axis, delta)) = turn {
                        carried.rotation.set(axis, wrap_degrees(other.start_rotation.get(axis) + delta));
                    }
                }
            }
            Handle::ResizeFace(axis, positive) => self.carry_size(scene, axis, positive, scale, &params),
            Handle::ResizeCorner(sides) => {
                for (axis, &side) in sides.iter().enumerate() {
                    self.carry_size(scene, axis, side, scale, &params);
                }
            }
        }
    }

    /// Size the carried nodes along `axis` as the dragged node has been.
    fn carry_size(
        &self,
        scene: &mut Scene,
        axis: usize,
        positive: bool,
        scale: simple3d_geom::Vec3,
        params: &simple3d_core::primitive::Params,
    ) {
        let start = self.start_extent(axis);
        let now = if self.gizmo.mode == Mode::Scale {
            start * scale.get(axis) / self.start_scale.get(axis)
        } else {
            match self.drivable(axis) {
                Some(driver) => params.num(driver.param) * driver.factor * self.gizmo.axis_scale[axis],
                None => return,
            }
        };
        let halves = if self.symmetric { 2.0 } else { 1.0 };
        for other in &self.others {
            let outward = if self.gizmo.mode == Mode::Scale {
                if start <= MIN_EXTENT {
                    continue;
                }
                other.start_extent(axis) * (now / start - 1.0)
            } else {
                now - start
            };
            other.size_axis(scene, axis, outward / halves, self.symmetric, positive);
        }
    }
}

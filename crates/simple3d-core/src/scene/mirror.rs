//! A node's mirror image (issue 122): the placement that stands it where its reflection across a plane
//! would be. A reflection is no turn, so one of the node's own axes takes it as a negative scale.

use super::*;
use crate::unit::wrap_degrees;
use crate::xform::Xform;

/// Where a mirrored node stands in its parent's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mirrored {
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
}

impl Node {
    /// This node reflected across the plane through `point` with normal `normal`, both in world space.
    /// `parent` is its parent's world frame. The axis flipped is the node's own one lying closest to the
    /// normal, so a box mirrored straight across keeps its turn.
    pub fn mirrored(&self, parent: Xform, point: Vec3, normal: Vec3) -> Mirrored {
        let scale = Node::sane_scale(self.scale);
        let own = Xform::from_pos_rot_scale(self.position, self.rotation, scale);
        let mirror = Xform::reflection(point, normal);
        let local = parent.inverse().compose(&mirror).compose(&parent).compose(&own);
        let world = parent.compose(&own);
        let normal = normal.normalized();
        let flipped =
            (0..3).max_by(|&a, &b| world.axis(a).dot(normal).abs().total_cmp(&world.axis(b).dot(normal).abs()));
        let mut sizes = Vec3::ZERO;
        let mut axes = [Vec3::ZERO; 3];
        for (axis, turned) in axes.iter_mut().enumerate() {
            let column = local.axis_vector(axis);
            let was = scale.get(axis);
            // Unchanged sizes stay exactly what they were, rather than picking up rounding.
            let size = if (column.length() - was.abs()).abs() < 1e-9 { was.abs() } else { column.length() };
            let sign = if (was < 0.0) != (Some(axis) == flipped) { -1.0 } else { 1.0 };
            sizes.set(axis, sign * size);
            *turned = column * (1.0 / (sign * size));
        }
        let rotation = Xform::rotation_of_axes(axes[0], axes[1], axes[2]);
        let tidy = |degrees: f64| wrap_degrees((degrees * 1e9).round() / 1e9);
        Mirrored {
            position: local.t,
            rotation: Vec3::new(tidy(rotation.x), tidy(rotation.y), tidy(rotation.z)),
            scale: Node::sane_scale(sizes),
        }
    }
}

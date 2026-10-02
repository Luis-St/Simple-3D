//! One shape found and nothing else: the mesh node becomes that shape, with no group around it.

use super::*;
use crate::app::App;
use simple3d_core::scene::{Anchor, Body, Node};
use simple3d_core::xform::Xform;
use simple3d_geom::reassemble::{Assembly, Part};
use simple3d_geom::Vec3;

impl App {
    /// Turn `target` itself into the assembly's one recognised shape, keeping its name, colour and
    /// place. `false` when there is more than that, or when the fit's turn cannot be folded into a
    /// non-uniformly scaled node, which is then left to the group.
    pub(super) fn become_single(&mut self, target: NodeId, assembly: &Assembly) -> bool {
        let ([group], None) = (&assembly.groups[..], &assembly.rest) else { return false };
        let [index] = group[..] else { return false };
        let part = &assembly.parts[index];
        let Some((type_id, params)) = super::run::recipe(part.shape) else { return false };
        let Some(node) = self.scene.get(target) else { return false };
        let Some(placed) = placement(node, part) else { return false };
        let node = self.scene.get_mut(target).expect("it was there a line ago");
        node.body = Body::Primitive { type_id: type_id.to_string(), params };
        node.segments = part.shape.segments();
        (node.position, node.rotation) = placed;
        node.anchor = Anchor::Centre;
        true
    }
}

/// The node's position and rotation holding `part` where the mesh's triangles stood: the mesh's
/// placement composed with the fit's. The base lift of a base-anchored mesh is folded in, since the
/// shape is centred.
fn placement(node: &Node, part: &Part) -> Option<(Vec3, Vec3)> {
    let scale = Node::sane_scale(node.scale);
    let lift = match (node.anchor, node.mesh().and_then(|mesh| mesh.mesh.bounds())) {
        (Anchor::Base, Some((lo, _))) => Vec3::new(0.0, 0.0, -lo.z),
        _ => Vec3::ZERO,
    };
    let own = Xform::from_pos_rot_scale(node.position, node.rotation, scale);
    let position = own.point(part.centre + lift);
    if part.rotation == Vec3::ZERO {
        return Some((position, node.rotation));
    }
    let uniform = (scale.x - scale.y).abs() < 1e-12 && (scale.y - scale.z).abs() < 1e-12;
    if !uniform {
        return None;
    }
    let turned =
        Xform::from_pos_rot(Vec3::ZERO, node.rotation).compose(&Xform::from_pos_rot(Vec3::ZERO, part.rotation));
    Some((position, euler_deg(&turned)))
}

/// A rotation matrix as a node's X-then-Y-then-Z angles in degrees (`Rz * Ry * Rx`). In gimbal lock
/// only the sum of the X and Z turns is recoverable, and Z is set to zero.
pub(crate) fn euler_deg(turn: &Xform) -> Vec3 {
    let m = turn.m;
    let sin_y = -m[2][0];
    let cos_y = m[0][0].hypot(m[1][0]);
    if cos_y < 1e-9 {
        let x = (-m[1][2]).atan2(m[1][1]);
        return Vec3::new(x.to_degrees(), sin_y.clamp(-1.0, 1.0).asin().to_degrees(), 0.0);
    }
    Vec3::new(m[2][1].atan2(m[2][2]).to_degrees(), sin_y.atan2(cos_y).to_degrees(), m[1][0].atan2(m[0][0]).to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_read_back_from_their_matrix_turn_the_same_way() {
        for angles in [Vec3::new(10.0, 20.0, 30.0), Vec3::new(-75.0, 40.0, 160.0), Vec3::new(0.0, 90.0, 0.0)] {
            let turn = Xform::from_pos_rot(Vec3::ZERO, angles);
            let back = Xform::from_pos_rot(Vec3::ZERO, euler_deg(&turn));
            for axis in 0..3 {
                assert!((turn.axis(axis) - back.axis(axis)).length() < 1e-9, "{angles:?} read back differently");
            }
        }
    }
}

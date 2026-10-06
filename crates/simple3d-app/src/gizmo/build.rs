//! The manipulator's geometry: where each handle sits in world space.

use super::*;
use crate::view::View;
use simple3d_core::eval::Evaluated;
use simple3d_core::primitive::AxisDriver;
use simple3d_core::scene::{Node, NodeId, Scene};
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;

/// The manipulator for one selected node, positioned in world space.
#[derive(Clone, Debug)]
pub struct Gizmo {
    pub mode: Mode,
    /// Where move and rotate handles centre: the node's origin, or a group's contents' middle
    /// (issue 90), since a group's origin can be far from its children.
    pub origin: Vec3,
    /// The node's own axes in world space, which the handles stand along.
    pub axes: [Vec3; 3],
    /// The node's own frame, for turning local box corners into world points.
    pub own: Xform,
    /// The parent frame, for writing a dragged world position back to `Node::position`.
    pub parent: Xform,
    pub local_lo: Vec3,
    pub local_hi: Vec3,
    /// Which parameter governs each local extent; `None` means no resize handle on that axis.
    pub drivers: [Option<AxisDriver>; 3],
    /// The node's own scale factors.
    pub own_scale: Vec3,
    /// World millimetres per local millimetre along each axis (own and ancestors' scale), to turn a
    /// screen drag into a dimension change.
    pub axis_scale: [f64; 3],
}

impl Gizmo {
    pub fn build(scene: &Scene, evaluated: &Evaluated, id: NodeId, mode: Mode) -> Option<Gizmo> {
        let node = scene.get(id)?;
        if id == scene.root() {
            return None;
        }
        let parent = *evaluated.node_frames.get(&id)?;
        let own =
            parent.compose(&Xform::from_pos_rot_scale(node.position, node.rotation, Node::sane_scale(node.scale)));
        // Always the node's own axes; the world-axes switch was removed (issue 100).
        let axes = [own.axis(0), own.axis(1), own.axis(2)];
        let (local_lo, local_hi) = evaluated.node_local_bounds.get(&id).copied().unwrap_or((Vec3::ZERO, Vec3::ZERO));
        let drivers = match (node.spec(), node.params()) {
            (Some(spec), Some(params)) => (spec.axes)(params),
            _ => [None, None, None],
        };
        let axis_scale = [0, 1, 2].map(|a| own.axis_vector(a).length().max(1e-9));
        // Measured on the last evaluation but placed with the live transform, so a group's handle keeps
        // up with a drag ahead of its evaluation.
        let origin = match (evaluated.node_world_bounds.get(&id), evaluated.placements.get(&id)) {
            (Some(&(lo, hi)), Some(measured)) if node.is_group() || node.is_split() => {
                own.point(parent.compose(measured).inverse().point((lo + hi) * 0.5))
            }
            _ => parent.point(node.position),
        };
        Some(Gizmo {
            mode,
            origin,
            axes,
            own,
            parent,
            local_lo,
            local_hi,
            drivers,
            own_scale: Node::sane_scale(node.scale),
            axis_scale,
        })
    }

    /// The handles to draw and hit-test, smaller and more specific first so corners beat faces.
    pub fn handles(&self, is_group: bool) -> Vec<Handle> {
        match self.mode {
            Mode::Move => {
                let mut out: Vec<Handle> = (0..3).map(Handle::MovePlane).collect();
                out.extend((0..3).map(Handle::MoveAxis));
                out
            }
            Mode::Rotate => (0..3).map(Handle::RotateRing).collect(),
            Mode::PushPull => Vec::new(),
            // Scale works on every axis and corner, for groups as well as primitives.
            Mode::Scale => {
                let mut out: Vec<Handle> = CORNERS.iter().map(|c| Handle::ResizeCorner(*c)).collect();
                for axis in 0..3 {
                    out.push(Handle::ResizeFace(axis, false));
                    out.push(Handle::ResizeFace(axis, true));
                }
                out
            }
            Mode::Resize => {
                // No resize handles on groups: resizing one truthfully rewrites every descendant (spec section 6.2).
                if is_group {
                    return Vec::new();
                }
                let mut out = Vec::new();
                for corner in CORNERS {
                    // A corner needs at least two drivable axes; one is a face handle's job.
                    if (0..3).filter(|&a| self.drivers[a].is_some()).count() >= 2 {
                        out.push(Handle::ResizeCorner(corner));
                    }
                }
                for axis in 0..3 {
                    if self.drivers[axis].is_some() {
                        out.push(Handle::ResizeFace(axis, false));
                        out.push(Handle::ResizeFace(axis, true));
                    }
                }
                out
            }
        }
    }

    /// The arm length in millimetres that keeps handles a constant size on screen.
    pub fn arm(&self, view: &View) -> f64 {
        ARM_PIXELS * view.mm_per_pixel_at(self.origin)
    }

    /// Where a handle sits in world space.
    pub fn handle_point(&self, handle: Handle, view: &View) -> Vec3 {
        let arm = self.arm(view);
        match handle {
            Handle::MoveAxis(a) => self.origin + self.axes[a] * arm,
            Handle::MovePlane(a) => {
                let (u, v) = other_axes(a);
                self.origin + (self.axes[u] + self.axes[v]) * (arm * PLANE_FRACTION)
            }
            Handle::RotateRing(a) => self.origin + self.axes[a] * arm,
            Handle::ResizeFace(a, positive) => self.own.point(self.face_centre(a, positive)),
            Handle::ResizeCorner(sides) => self.own.point(self.corner(sides)),
        }
    }

    /// The `Node::position` keeping `origin` in place under a new `rotation` and `scale`, so a group
    /// turns about its drawn middle rather than an off-centre origin.
    pub fn position_keeping_pivot(&self, rotation: Vec3, scale: Vec3) -> Vec3 {
        let pivot_local = self.own.inverse().point(self.origin);
        let turned = Xform::from_pos_rot_scale(Vec3::ZERO, rotation, Node::sane_scale(scale));
        self.parent.inverse().point(self.origin) - turned.vector(pivot_local)
    }

    pub(super) fn face_centre(&self, axis: usize, positive: bool) -> Vec3 {
        let mid = (self.local_lo + self.local_hi) * 0.5;
        let edge = if positive { self.local_hi } else { self.local_lo };
        let mut p = mid;
        p.set(axis, edge.get(axis));
        p
    }

    pub(super) fn corner(&self, sides: [bool; 3]) -> Vec3 {
        let mut p = Vec3::ZERO;
        for axis in 0..3 {
            let edge = if sides[axis] { self.local_hi } else { self.local_lo };
            p.set(axis, edge.get(axis));
        }
        p
    }
}

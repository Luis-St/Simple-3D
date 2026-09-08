//! A drag on a handle, from the moment it is taken hold of.

use super::*;
use crate::view::View;
use simple3d_core::primitive::Params;
use simple3d_core::scene::{Node, NodeId, Scene};
use simple3d_geom::Vec3;

/// An in-progress drag. Everything needed to reproduce the node's pre-drag state
/// exactly, so Escape restores it and the whole drag is one undo step.
#[derive(Clone, Debug)]
pub struct Drag {
    pub node: NodeId,
    pub handle: Handle,
    /// The manipulator **as it stood when the handle was grabbed**.
    ///
    /// A drag must be measured against a frame that does not move, and the live
    /// gizmo is rebuilt every frame from the very node the drag is moving. Using
    /// it here meant the reference slid along with the result: once the node had
    /// moved 10 mm, `gizmo.origin` had moved 10 mm too, the cursor measured 10 mm
    /// nearer, and the next frame wrote the node back to where it started. The
    /// position then flipped between the two on alternate frames, and where a
    /// drag finished came down to which frame the button happened to come up on.
    pub(super) gizmo: Gizmo,
    pub start_position: Vec3,
    pub start_rotation: Vec3,
    pub start_scale: Vec3,
    pub start_params: Params,
    pub start_local_lo: Vec3,
    pub start_local_hi: Vec3,
    /// The scalar the handle was grabbed at, in whatever the handle measures.
    pub(super) grab: f64,
    /// Where a plane handle was grabbed, in world space.
    pub(super) grab_point: Vec3,
    /// Accumulated rotation angle, unwrapped across the +/-180 degree seam.
    pub(super) last_angle: f64,
    pub(super) turns: f64,
    /// The last value shown at the cursor.
    pub readout: String,
}

impl Drag {
    pub fn begin(
        scene: &Scene,
        gizmo: &Gizmo,
        node: NodeId,
        handle: Handle,
        view: &View,
        cursor: egui::Pos2,
    ) -> Option<Drag> {
        let n = scene.get(node)?;
        let mut drag = Drag {
            node,
            handle,
            gizmo: gizmo.clone(),
            start_position: n.position,
            start_rotation: n.rotation,
            start_scale: Node::sane_scale(n.scale),
            start_params: n.params().cloned().unwrap_or_default(),
            start_local_lo: gizmo.local_lo,
            start_local_hi: gizmo.local_hi,
            grab: 0.0,
            grab_point: Vec3::ZERO,
            last_angle: 0.0,
            turns: 0.0,
            readout: String::new(),
        };
        match handle {
            Handle::MoveAxis(a) => {
                drag.grab = view.ray_axis(cursor, gizmo.origin, gizmo.axes[a])?;
            }
            Handle::MovePlane(a) => {
                drag.grab_point = view.ray_plane(cursor, gizmo.origin, gizmo.axes[a])?;
            }
            Handle::RotateRing(a) => {
                drag.last_angle = ring_angle(gizmo, a, view, cursor)?;
                drag.grab = drag.last_angle;
            }
            Handle::ResizeFace(a, positive) => {
                let anchor = gizmo.own.point(gizmo.face_centre(a, positive));
                drag.grab = view.ray_axis(cursor, anchor, gizmo.axes[a])?;
            }
            Handle::ResizeCorner(sides) => {
                let anchor = gizmo.own.point(gizmo.corner(sides));
                // A corner is dragged in the plane facing the camera, and each
                // axis takes the component of that movement along itself.
                drag.grab_point = view.ray_plane(cursor, anchor, view.forward())?;
            }
        }
        Some(drag)
    }
}

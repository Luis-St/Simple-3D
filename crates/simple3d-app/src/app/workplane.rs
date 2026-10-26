//! The active workplane as the rest of the app sees it (issue 125): where new shapes land and how
//! they are turned, the plane a pick falls back on, the grid drawn and snapped to, and the axes the
//! move handles stand along. With no workplane active every answer is the ground's.

use super::*;
use crate::render::GridPlane;
use crate::view::View;
use simple3d_core::scene::{Node, NodeId, Workplane};
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;

/// The sketch grid's finest spacing, in millimetres.
const SKETCH_GRID: f64 = 1.0;

impl App {
    /// The workplane shapes are added, moved and snapped on, or `None` for the ground.
    pub(crate) fn workplane(&self) -> Option<&Workplane> {
        self.scene.settings.active_workplane()
    }

    /// The grid drawn and snapped to, when it is not the ground: the sketch plane while sketching, the
    /// workplane being placed while the tool has a face, else the active one.
    pub(crate) fn grid_plane(&self) -> Option<GridPlane> {
        // While sketching the grid is the sketch plane's, 1 mm fine with a heavier line every grid
        // spacing (issue 74).
        if let Some(frame) = self.sketch_tool.as_ref().and_then(|tool| tool.frame) {
            let every = (self.scene.settings.grid_spacing / SKETCH_GRID).round().clamp(2.0, 1000.0) as u32;
            return Some(GridPlane {
                origin: frame.t,
                u: frame.axis(0),
                v: frame.axis(1),
                spacing: SKETCH_GRID,
                major_every: every,
                emphasise: true,
            });
        }
        let placing = self.workplane_tool.as_ref().and_then(|tool| tool.preview());
        placing.or_else(|| self.workplane().cloned()).map(|plane| GridPlane::of(&plane))
    }

    /// The origin axes drawn: the document's, less the one pointing straight out of the sketch plane
    /// while sketching, since the sketch is a 2D view and that axis is only a dot or a stray line.
    pub(crate) fn shown_axes(&self) -> [bool; 3] {
        let mut axes = self.scene.settings.axes_visible;
        if let Some(frame) = self.sketch_tool.as_ref().and_then(|tool| tool.frame) {
            let normal = frame.axis(2);
            for (axis, shown) in axes.iter_mut().enumerate() {
                let along = [normal.x, normal.y, normal.z][axis].abs();
                if along > 1.0 - 1e-6 {
                    *shown = false;
                }
            }
        }
        axes
    }

    /// Where a new shape lands when nothing else says: the workplane's origin, or the world's.
    pub(crate) fn work_origin(&self) -> Vec3 {
        self.workplane().map_or(Vec3::ZERO, Workplane::origin)
    }

    /// The point on the drawn grid's plane under the pointer, the ground's without a workplane.
    pub(crate) fn work_ground_under(&self, view: &View, cursor: egui::Pos2) -> Option<Vec3> {
        let plane = self.grid_plane().unwrap_or_else(|| GridPlane::ground(1.0));
        plane.under(view, cursor)
    }

    /// The `Node::rotation` under `parent` that lays a new shape on the active workplane, or `None`
    /// on the ground, where shapes keep their own.
    pub(crate) fn work_rotation_in(&self, parent: NodeId) -> Option<Vec3> {
        let plane = self.workplane()?;
        let turn = Xform::from_pos_rot(Vec3::ZERO, plane.rotation());
        let local = self.child_frame(parent).inverse().compose(&turn);
        Some(Xform::rotation_of_axes(local.axis(0), local.axis(1), local.axis(2)))
    }

    /// The frame a child of `parent` lives in: the parent's own placement in its frame.
    pub(crate) fn child_frame(&self, parent: NodeId) -> Xform {
        match self.evaluated.node_frames.get(&parent) {
            Some(frame) => {
                let node = self.scene.node(parent);
                frame.compose(&Xform::from_pos_rot_scale(node.position, node.rotation, Node::sane_scale(node.scale)))
            }
            None => Xform::IDENTITY,
        }
    }
}

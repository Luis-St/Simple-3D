//! Where the scene is looked at from: framing, presets and turning.

use super::*;
use crate::gizmo::Gizmo;
use crate::view::{frame_bounds, CameraMove, ViewPreset};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    // -- camera -------------------------------------------------------------

    pub(super) fn aspect(&self) -> f64 {
        let size = self.viewport_rect.size();
        if size.y > 1.0 {
            (size.x / size.y) as f64
        } else {
            1.0
        }
    }

    /// Frame `lo`..`hi`, honouring the view-centre lock: a pinned centre keeps
    /// its place and only the zoom changes.
    ///
    /// Framing is the one command whose whole job is to move the view centre, so
    /// a lock could as easily have disabled it. It fits the zoom instead, and
    /// says which half it did: a Frame button that does nothing at all reads as
    /// a broken button, and half of framing is still worth having.
    pub(super) fn frame_onto(&mut self, lo: Vec3, hi: Vec3) {
        let aspect = self.aspect();
        let pinned = self.settings.lock_view_centre.then_some(self.scene.camera.target);
        frame_bounds(&mut self.scene.camera, lo, hi, aspect);
        if let Some(target) = pinned {
            self.scene.camera.target = target;
            self.status = Status::Info("The view centre is locked, so framing changed the zoom only".into());
        }
    }

    pub fn frame_all(&mut self) {
        match self.evaluated.mesh.bounds().or_else(|| self.selection_bounds()) {
            Some((lo, hi)) => self.frame_onto(lo, hi),
            None => {
                if !self.settings.lock_view_centre {
                    self.scene.camera.target = Vec3::ZERO;
                }
                self.scene.camera.distance = 160.0;
            }
        }
    }

    pub fn frame_selection(&mut self) {
        match self.selection_bounds() {
            Some((lo, hi)) => self.frame_onto(lo, hi),
            None => self.frame_all(),
        }
    }

    pub fn selection_bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut result: Option<(Vec3, Vec3)> = None;
        for id in &self.selection {
            for node in std::iter::once(*id).chain(self.scene.descendants(*id)) {
                let Some(mesh) = self.evaluated.node_meshes.get(&node) else { continue };
                let Some((lo, hi)) = mesh.bounds() else { continue };
                result = Some(match result {
                    None => (lo, hi),
                    Some((a, b)) => (a.min(lo), b.max(hi)),
                });
            }
        }
        result
    }

    pub fn set_view(&mut self, preset: ViewPreset) {
        let (yaw, pitch) = preset.angles();
        self.turn_camera_to(yaw, pitch);
        self.status = Status::Info(format!("View: {}", preset.label()));
    }

    /// Turn the camera to face a given way, over the design's 200 ms, taking
    /// the short way round. Under a reduced-motion preference it simply arrives:
    /// the transition is there to show that this is the same camera moving, and
    /// someone who does not want things moving does not need to be shown that.
    pub fn turn_camera_to(&mut self, yaw: f64, pitch: f64) {
        let from = (self.scene.camera.yaw, self.scene.camera.pitch);
        let to = (from.0 + crate::view::shortest_turn(from.0, yaw), pitch);
        if self.settings.reduce_motion {
            self.scene.camera.yaw = to.0;
            self.scene.camera.pitch = to.1;
            self.camera_move = None;
            return;
        }
        self.camera_move = Some(CameraMove { from, to, started: std::time::Instant::now() });
    }

    /// Advance a view change. Called once a frame; does nothing when none is in
    /// flight.
    pub fn advance_camera(&mut self) {
        let Some(move_) = self.camera_move else { return };
        let ((yaw, pitch), done) = move_.at(std::time::Instant::now());
        self.scene.camera.yaw = yaw;
        self.scene.camera.pitch = pitch;
        if done {
            self.camera_move = None;
        }
    }
}

impl App {
    pub fn gizmo_for(&self, id: NodeId) -> Option<Gizmo> {
        Gizmo::build(&self.scene, &self.evaluated, id, self.mode)
    }

    pub fn current_view(&self) -> crate::view::View {
        crate::view::View::new(self.scene.camera, self.viewport_rect)
    }
}

//! Where the scene is viewed from: framing, presets and turning.

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

    /// Frame `lo`..`hi`; with the view centre locked only the zoom changes, and the status says so
    /// rather than the button seeming broken.
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
        match self.evaluated.bounds.or_else(|| self.selection_bounds()) {
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
        let mut grow = |(lo, hi): (Vec3, Vec3)| {
            result = Some(match result {
                None => (lo, hi),
                Some((a, b)) => (a.min(lo), b.max(hi)),
            });
        };
        for id in &self.selection {
            // The evaluated size: a difference is what is left, not its cutters.
            if let Some(&bounds) = self.evaluated.node_world_bounds.get(id) {
                grow(bounds);
                continue;
            }
            for node in std::iter::once(*id).chain(self.scene.descendants(*id)) {
                // Nodes with their own mesh, measured by the evaluation.
                if !self.evaluated.node_meshes.contains_key(&node) {
                    continue;
                }
                let Some(&bounds) = self.evaluated.node_world_bounds.get(&node) else { continue };
                grow(bounds);
            }
        }
        result
    }

    pub fn set_view(&mut self, preset: ViewPreset) {
        let (yaw, pitch) = preset.angles();
        self.turn_camera_to(yaw, pitch);
        self.status = Status::Info(format!("View: {}", preset.label()));
    }

    /// Turn the camera over 200 ms the short way round; with reduced motion it arrives at once.
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

    /// Advance a view change, once per frame.
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

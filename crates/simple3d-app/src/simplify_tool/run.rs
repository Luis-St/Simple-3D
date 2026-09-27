//! Keeping the result, and putting the mesh back.

use super::*;
use crate::app::{App, Status};

impl App {
    /// Keep what is on screen (issue 106). The preview already holds the result, so the original is put
    /// back for the undo snapshot and the result written again; snapshotting the preview would undo to it.
    pub fn apply_simplify(&mut self) {
        let ready =
            self.simplify_tool.as_ref().is_some_and(|tool| tool.shown.is_some() && self.scene.contains(tool.target));
        if !ready {
            return;
        }
        let tool = self.simplify_tool.take().expect("it was there a line ago");
        let shown = tool.shown.expect("it was there a line ago");
        if let Some(job) = &tool.job {
            job.cancel();
        }
        let name = self.scene.node(tool.target).name.clone();
        let before = tool.original.triangle_count();
        let after = shown.mesh.triangle_count();
        self.scene.set_mesh(tool.target, tool.original.clone());
        self.edit("Simplify the mesh", None);
        self.scene.set_mesh(tool.target, shown.mesh);
        self.touch();
        self.settings.last_simplify = tool.plan;
        self.persist();
        self.status = Status::Info(format!(
            "Simplified {name} from {before} triangles to {after}, moving the surface at most {} -- {}",
            distance(shown.deviation, self.unit()),
            self.way_back()
        ));
    }

    /// Close the tool and restore the mesh.
    pub fn cancel_simplify_tool(&mut self) {
        let Some(tool) = self.simplify_tool.take() else { return };
        if let Some(job) = &tool.job {
            job.cancel();
        }
        // Only if a preview is standing: otherwise nothing changed, and restoring would cost a full evaluation.
        if tool.shown.is_some() && self.scene.contains(tool.target) {
            self.scene.set_mesh(tool.target, tool.original);
            self.touch();
        }
    }
}

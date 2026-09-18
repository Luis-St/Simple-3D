//! Keeping the result, and putting the mesh back.

use super::*;
use crate::app::{App, Status};

impl App {
    /// Keep what is on screen (issue 106).
    ///
    /// The document already holds the simplified mesh -- it is what the preview
    /// put there -- so nothing is computed here. What happens is the undo step:
    /// the mesh that was there before the preview goes back in, the snapshot is
    /// taken over *that*, and the result is written again on top of it. Taking
    /// the snapshot over the preview instead would leave undo stepping back to
    /// a simplification nobody asked for.
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

    /// Put the tool away, and the mesh back as it was.
    pub fn cancel_simplify_tool(&mut self) {
        let Some(tool) = self.simplify_tool.take() else { return };
        if let Some(job) = &tool.job {
            job.cancel();
        }
        // Only when a preview was actually standing in the document: a tool
        // closed before its first run finished has changed nothing, and writing
        // the mesh back would cost an evaluation of the whole scene to arrive
        // at the scene it already is.
        if tool.shown.is_some() && self.scene.contains(tool.target) {
            self.scene.set_mesh(tool.target, tool.original);
            self.touch();
        }
    }
}

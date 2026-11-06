//! Keeping the result, and putting the mesh back.

use crate::app::{App, Status};

impl App {
    /// Keep what is on screen (issue 81). The original is put back for the undo snapshot and the result
    /// written again, so undo returns to the mesh as it was, not to the preview.
    pub fn apply_shape(&mut self) {
        let ready =
            self.shape_tool.as_ref().is_some_and(|tool| tool.shown.is_some() && self.scene.contains(tool.target));
        if !ready {
            return;
        }
        let tool = self.shape_tool.take().expect("it was there a line ago");
        let shown = tool.shown.expect("it was there a line ago");
        if let Some(job) = &tool.job {
            job.cancel();
        }
        let name = self.scene.node(tool.target).name.clone();
        let function = tool.plan.function.label().to_lowercase();
        self.scene.set_mesh(tool.target, tool.original.clone());
        self.edit(&format!("Apply {function} to the mesh"), None);
        self.scene.set_mesh(tool.target, shown.mesh);
        self.touch();
        self.settings.last_shaping = tool.plan;
        self.persist();
        self.status = Status::Info(format!(
            "Applied {function} to {name}, moving the surface at most {} -- {}",
            crate::simplify_tool::distance(shown.moved, self.unit()),
            self.way_back()
        ));
    }

    /// Close the tool and restore the mesh.
    pub fn cancel_shape_tool(&mut self) {
        let Some(tool) = self.shape_tool.take() else { return };
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

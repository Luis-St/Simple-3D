//! Opening the tool on a mesh.

use super::*;
use crate::app::{App, Status};

impl App {
    /// Open the tool on the selection (issue 81): one mesh only, as for Simplify, since a primitive's
    /// parameters would not survive its surface being moved.
    pub fn open_shape_tool(&mut self) {
        let Some(id) =
            self.single_target("Select a mesh to apply a function to", "Apply a function to one mesh at a time")
        else {
            return;
        };
        let Some(mesh) = self.scene.get(id).and_then(|node| node.mesh()).cloned() else {
            self.status = Status::Warning("A function only runs over a mesh -- convert this to a mesh first".into());
            return;
        };
        // The two mesh tools share the one preview (`crate::mesh_preview`).
        self.cancel_mesh_tools();
        self.shape_tool =
            Some(ShapeTool { target: id, original: mesh, plan: self.settings.last_shaping, shown: None, job: None });
    }
}

//! Opening the tool on a mesh.

use super::*;
use crate::app::{App, Status};
use simple3d_geom::simplify::MIN_TRIANGLES;

impl App {
    /// Open the tool on the selection (issue 106): one mesh only. Primitives and groups are refused
    /// because their parameters would be lost; Convert to mesh says so as its own step.
    pub fn open_simplify_tool(&mut self) {
        let Some(id) = self.single_target("Select a mesh to simplify", "Simplify one mesh at a time") else { return };
        let Some(mesh) = self.scene.get(id).and_then(|node| node.mesh()).cloned() else {
            self.status = Status::Warning("Only a mesh can be simplified -- convert this to a mesh first".into());
            return;
        };
        if mesh.triangle_count() <= MIN_TRIANGLES {
            self.status = Status::Warning("There is no detail there to drop".into());
            return;
        }
        self.simplify_tool = Some(SimplifyTool {
            target: id,
            original: mesh,
            plan: self.settings.last_simplify,
            shown: None,
            job: None,
            wireframe: true,
        });
    }
}

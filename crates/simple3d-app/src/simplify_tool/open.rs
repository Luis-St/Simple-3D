//! Opening the tool on a mesh.

use super::*;
use crate::app::{App, Status};
use simple3d_geom::simplify::MIN_TRIANGLES;

impl App {
    /// Open the tool on the selection (issue 106).
    ///
    /// Only on a mesh, and only on one at a time. A primitive or a group is not
    /// refused for want of a way to simplify it -- it could be baked and
    /// simplified in one gesture -- but because what would come back is not the
    /// shape that was selected: the parameters behind it would be gone, and a
    /// box that is no longer 40 by 30 by 10 is a different kind of loss from a
    /// mesh with fewer triangles in it. Convert to a mesh says that in its own
    /// step, and this one then says what it costs.
    pub fn open_simplify_tool(&mut self) {
        let targets = self.top_level_selection();
        let Some(&id) = targets.first() else {
            self.status = Status::Warning("Select a mesh to simplify".into());
            return;
        };
        if targets.len() > 1 {
            self.status = Status::Warning("Simplify one mesh at a time".into());
            return;
        }
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

//! Opening the tool on a shape, and keeping it in step as the shape changes.

use super::*;
use crate::app::{App, Status};
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::Mesh;
use std::sync::Arc;

impl App {
    /// Open the tool on the selection (issue 82), on the last split's numbers, or on a split's own
    /// stored plan so cutting again changes its pattern.
    pub fn open_split_tool(&mut self) {
        let targets = self.top_level_selection();
        let Some(&id) = targets.first() else {
            self.status = Status::Warning("Select something to split into pieces".into());
            return;
        };
        if targets.len() > 1 {
            self.status = Status::Warning("Split one object at a time".into());
            return;
        }
        if self.split_job.is_some() {
            self.status = Status::Warning("A split is already running".into());
            return;
        }
        let (mesh, placement) = self.bake_for_split(id);
        let Some(bounds) = mesh.bounds() else {
            self.status = Status::Warning("There is no geometry there to split".into());
            return;
        };
        let plan = self.scene.node(id).split_plan().cloned().unwrap_or_else(|| self.settings.last_split.clone());
        self.split_tool = Some(SplitTool {
            target: id,
            mesh: Arc::new(mesh),
            bounds,
            placement,
            generation: self.evaluation_generation,
            plan,
        });
    }

    /// The shape to cut in its own frame, and that frame's world placement for drawing cells.
    pub(super) fn bake_for_split(&self, id: NodeId) -> (Mesh, Xform) {
        let parent = self.evaluated.node_frames.get(&id).copied().unwrap_or(Xform::IDENTITY);
        simple3d_core::eval::baked_mesh_in_place(&self.scene, id, parent)
    }

    /// Sync the open non-modal tool with the document: close if the shape is deleted, re-bake if edited.
    pub(crate) fn refresh_split_tool(&mut self) {
        let Some(tool) = self.split_tool.as_ref() else { return };
        if !self.scene.contains(tool.target) {
            self.cancel_split_tool();
            self.status = Status::Warning("The object being split is no longer there".into());
            return;
        }
        if tool.generation == self.evaluation_generation {
            return;
        }
        let target = tool.target;
        let (mesh, placement) = self.bake_for_split(target);
        let tool = self.split_tool.as_mut().expect("it was there a line ago");
        tool.generation = self.evaluation_generation;
        // The placement follows the shape regardless, so the cells move with a moved shape.
        tool.placement = placement;
        // A shape edited to nothing keeps the last numbers up; Split refuses on its own.
        if let Some(bounds) = mesh.bounds() {
            tool.bounds = bounds;
            tool.mesh = Arc::new(mesh);
        }
    }
}

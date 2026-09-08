//! Opening the tool on a shape, and keeping it in step with one that
//! changes under it.

use super::*;
use crate::app::{App, Status};
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::Mesh;
use std::sync::Arc;

impl App {
    /// Open the tool on the selection (issue 82).
    ///
    /// It opens on the numbers the last split used rather than on a default, so
    /// cutting a second shape the same way is one gesture; what a split was cut
    /// with is also stored on the split itself, so opening the tool on one
    /// offers *its* pattern and cutting again changes the pattern rather than
    /// splitting a split.
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

    /// The shape to be cut, in its own frame, and where that frame stands in
    /// the world -- one for the cutting and the numbers, the other for drawing
    /// the cells on the model.
    pub(super) fn bake_for_split(&self, id: NodeId) -> (Mesh, Xform) {
        let parent = self.evaluated.node_frames.get(&id).copied().unwrap_or(Xform::IDENTITY);
        simple3d_core::eval::baked_mesh_in_place(&self.scene, id, parent)
    }

    /// Keep the open tool honest against a document that can change underneath
    /// it, which a non-modal window can.
    ///
    /// Three things can happen to the shape while the tool is up: it can be
    /// deleted, which closes the tool; it can be edited, which re-bakes what the
    /// picture is drawn from; and it can be left alone, which is the usual case
    /// and costs one integer comparison.
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
        // The placement follows the shape whatever happens to the geometry: a
        // shape merely moved is the same cut in a new place, and the cells have
        // to move with it.
        tool.placement = placement;
        // A shape edited down to nothing -- hidden, or emptied of children --
        // leaves the last numbers up rather than blanking the window: they are
        // still worth reading, and Split refuses on its own.
        if let Some(bounds) = mesh.bounds() {
            tool.bounds = bounds;
            tool.mesh = Arc::new(mesh);
        }
    }
}

//! Keeping the preview in step with the numbers.

use super::*;
use crate::app::{App, Status};

impl App {
    /// Sync the open tool with the document and the window's numbers: close if the mesh is gone,
    /// follow outside changes to it, put finished runs in the document, and start new ones.
    pub(crate) fn refresh_shape_tool(&mut self) {
        let Some(mut tool) = self.shape_tool.take() else { return };
        let held = self.scene.get(tool.target).and_then(|node| node.mesh()).cloned();
        let Some(held) = held else {
            if let Some(job) = &tool.job {
                job.cancel();
            }
            self.status = Status::Warning("The mesh the function was running over is no longer there".into());
            return;
        };
        // What the tool believes is on the node: its preview, or the original before the first run. Any
        // other mesh came from outside (an undo, a paste), and the tool follows it.
        let ours = match &tool.shown {
            Some(shown) => Arc::ptr_eq(&held, &shown.mesh),
            None => Arc::ptr_eq(&held, &tool.original),
        };
        if !ours {
            if let Some(job) = &tool.job {
                job.cancel();
            }
            tool = ShapeTool { original: held, shown: None, job: None, ..tool };
        }
        self.take_shape_run(&mut tool);
        if !tool.shown.as_ref().is_some_and(|shown| shown.plan == tool.plan)
            && ShapeJob::idle_for(tool.job.as_ref(), &tool.plan)
        {
            tool.job = Some(ShapeJob::start(tool.original.clone(), tool.plan));
        }
        self.shape_tool = Some(tool);
    }

    /// Put a finished run in the document, so the viewport shows the real result.
    fn take_shape_run(&mut self, tool: &mut ShapeTool) {
        let Some(job) = tool.job.as_ref() else { return };
        let Some(outcome) = job.poll() else { return };
        let plan = job.plan;
        tool.job = None;
        // An abandoned run has no answer; the next frame starts its replacement.
        let Some(outcome) = outcome else { return };
        let mesh = Arc::new(MeshData::new(outcome.mesh));
        if self.scene.set_mesh(tool.target, mesh.clone()) {
            self.touch();
            tool.shown = Some(Shown { plan, mesh, capped: outcome.capped, moved: outcome.moved });
        }
    }
}

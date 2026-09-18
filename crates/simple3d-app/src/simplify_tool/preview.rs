//! Keeping the preview in step with the numbers, and drawing its triangles.

use super::*;
use crate::app::{App, Status};
use simple3d_geom::Vec3;

impl App {
    /// Keep the open tool honest against a document that can change underneath
    /// it, and keep the shape in the viewport in step with the numbers in the
    /// window.
    ///
    /// Four things can have happened since the last frame: the mesh can have
    /// gone, which closes the tool; it can have been changed by something else
    /// -- an undo, a paste -- which makes *that* the mesh to simplify; a run
    /// can have finished, which is put in the document; and a number can have
    /// been turned, which starts the next run. The usual case is none of them,
    /// and costs two comparisons.
    pub(crate) fn refresh_simplify_tool(&mut self) {
        let Some(mut tool) = self.simplify_tool.take() else { return };
        let held = self.scene.get(tool.target).and_then(|node| node.mesh()).cloned();
        let Some(held) = held else {
            if let Some(job) = &tool.job {
                job.cancel();
            }
            self.status = Status::Warning("The mesh being simplified is no longer there".into());
            return;
        };
        // What the tool believes is on the node: the preview it put there, or,
        // before the first run lands, the mesh it opened on. Anything else came
        // from outside the tool, and the tool follows it rather than arguing
        // with it -- an undo while the window is open is a perfectly reasonable
        // thing to do, and what it undid is now what there is to simplify.
        let ours = match &tool.shown {
            Some(shown) => Arc::ptr_eq(&held, &shown.mesh),
            None => Arc::ptr_eq(&held, &tool.original),
        };
        if !ours {
            if let Some(job) = &tool.job {
                job.cancel();
            }
            tool = SimplifyTool { original: held, shown: None, job: None, ..tool };
        }
        self.take_simplify_run(&mut tool);
        self.start_simplify_run(&mut tool);
        self.simplify_tool = Some(tool);
    }

    /// Put a finished run in the document, so what is in the viewport is the
    /// result rather than a picture of one.
    fn take_simplify_run(&mut self, tool: &mut SimplifyTool) {
        let Some(job) = tool.job.as_ref() else { return };
        let Some(outcome) = job.poll() else { return };
        let plan = job.plan;
        tool.job = None;
        // An abandoned run has no answer, and the plan it was abandoned for is
        // already the one on screen: the next frame starts the run that
        // replaces it.
        let Some(outcome) = outcome else { return };
        let mesh = Arc::new(MeshData::new(outcome.mesh));
        if self.scene.set_mesh(tool.target, mesh.clone()) {
            self.touch();
            tool.shown = Some(Shown { plan, mesh, deviation: outcome.deviation });
        }
    }

    /// Start the run the numbers on screen are asking for, if it is not the one
    /// already showing or already running.
    ///
    /// Every run is computed from the mesh the tool opened on, never from the
    /// preview standing in the document: simplifying a simplification compounds
    /// what each one gave away, and a percentage turned back up would then
    /// never recover the detail it dropped on the way down.
    fn start_simplify_run(&mut self, tool: &mut SimplifyTool) {
        if tool.shown.as_ref().is_some_and(|shown| shown.plan == tool.plan) {
            return;
        }
        match tool.job.as_ref() {
            // One at a time. A scrubbed field asks for a new result on every
            // frame it moves, and what is wanted is the newest of them: the run
            // in flight is told to stop, and the next frame -- once it has --
            // starts the one that is actually wanted.
            Some(job) if job.plan == tool.plan => return,
            Some(job) => {
                job.cancel();
                return;
            }
            None => {}
        }
        tool.job = Some(SimplifyJob::spawn(tool.original.clone(), tool.plan));
    }
}

/// The triangles of the result, in world space, for the renderer to draw over
/// the shape (issue 106).
///
/// The mesh they are read from is the *evaluated* one rather than the tool's
/// own, which is what puts them on the model rather than beside it: the preview
/// is in the document, so the evaluation has already carried it through the
/// node's position, rotation and scale, and there is no second transform here
/// to fall out of step with the first.
///
/// They go to the renderer rather than to the 2D painter so the depth buffer
/// can have them: a triangle on the far side of the shape is behind it, and a
/// wireframe drawn through the solid reads as floating in front of it.
pub(crate) fn preview_loops(app: &App) -> Vec<Vec<Vec3>> {
    let Some(tool) = app.simplify_tool.as_ref() else { return Vec::new() };
    if !tool.wireframe || tool.shown.is_none() {
        return Vec::new();
    }
    let Some(mesh) = app.evaluated.node_meshes.get(&tool.target) else { return Vec::new() };
    if mesh.triangle_count() > WIREFRAME_LIMIT {
        return Vec::new();
    }
    mesh.indices.iter().map(|tri| tri.iter().map(|&v| mesh.positions[v as usize]).collect()).collect()
}

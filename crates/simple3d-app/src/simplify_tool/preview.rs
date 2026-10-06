//! Keeping the preview in step with the numbers, and drawing its triangles.

use super::*;
use crate::app::{App, Status};
use simple3d_geom::Vec3;

impl App {
    /// Sync the open tool with the document and the window's numbers: close if the mesh is gone,
    /// follow outside changes to it, put finished runs in the document, and start new ones.
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
            tool = SimplifyTool { original: held, shown: None, job: None, ..tool };
        }
        self.take_simplify_run(&mut tool);
        self.start_simplify_run(&mut tool);
        self.simplify_tool = Some(tool);
    }

    /// Put a finished run in the document, so the viewport shows the real result.
    fn take_simplify_run(&mut self, tool: &mut SimplifyTool) {
        let Some(job) = tool.job.as_ref() else { return };
        let Some(outcome) = job.poll() else { return };
        let plan = job.plan;
        tool.job = None;
        // An abandoned run has no answer; the next frame starts its replacement.
        let Some(outcome) = outcome else { return };
        let mesh = Arc::new(MeshData::new(outcome.mesh));
        if self.scene.set_mesh(tool.target, mesh.clone()) {
            self.touch();
            tool.shown = Some(Shown { plan, mesh, deviation: outcome.deviation });
        }
    }

    /// Start the run the numbers ask for, unless already showing or running. Always from the original
    /// mesh, so errors do not compound and raising the percentage recovers detail.
    fn start_simplify_run(&mut self, tool: &mut SimplifyTool) {
        if tool.shown.as_ref().is_some_and(|shown| shown.plan == tool.plan) {
            return;
        }
        if SimplifyJob::idle_for(tool.job.as_ref(), &tool.plan) {
            tool.job = Some(SimplifyJob::start(tool.original.clone(), tool.plan));
        }
    }
}

/// The result's triangles in world space for the renderer (issue 106), read from the evaluated
/// mesh so they sit on the model, and depth-tested so the far side is hidden.
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

impl App {
    /// The mesh `id` really holds: the original while the tool previews on it, so the Properties panel
    /// does not report a result nobody has applied.
    pub(crate) fn committed_mesh(&self, id: NodeId) -> Option<Arc<MeshData>> {
        match &self.simplify_tool {
            Some(tool) if tool.target == id => Some(tool.original.clone()),
            _ => self.scene.get(id)?.mesh().cloned(),
        }
    }

    /// `id`'s world bounds as committed: the original mesh placed as the evaluation places the preview.
    pub(crate) fn committed_world_bounds(&self, id: NodeId) -> Option<(Vec3, Vec3)> {
        let previewed = self.simplify_tool.as_ref().filter(|tool| tool.target == id && tool.shown.is_some());
        let Some(tool) = previewed else { return self.evaluated.node_world_bounds.get(&id).copied() };
        let node = self.scene.get(id)?;
        let placement = simple3d_core::xform::Xform::from_pos_rot_scale(
            node.position,
            node.rotation,
            simple3d_core::scene::Node::sane_scale(node.scale),
        );
        let own = self.evaluated.node_frames.get(&id)?.compose(&placement);
        let mesh = &tool.original.mesh;
        let (lo, _) = mesh.bounds()?;
        // As the evaluation lifts a base-anchored shape onto its base.
        let lift = match node.anchor {
            simple3d_core::scene::Anchor::Base => Vec3::new(0.0, 0.0, -lo.z),
            simple3d_core::scene::Anchor::Centre => Vec3::ZERO,
        };
        let mut points = mesh.positions.iter().map(|&p| own.point(p + lift));
        let first = points.next()?;
        Some(points.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }
}

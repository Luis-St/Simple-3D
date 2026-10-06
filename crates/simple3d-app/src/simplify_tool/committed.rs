//! The model's numbers as committed while a preview stands in the document: the document evaluated a
//! second time with the original mesh put back, since what a boolean makes of the original cannot be
//! worked out from the preview's numbers.

use crate::app::App;
use crate::worker::EvalWorker;
use simple3d_geom::Vec3;

/// The second evaluation, alive only as long as the tool, so its thread ends with it.
pub struct Committed {
    worker: EvalWorker,
    /// The latest answer; until the first one the callers fall back to estimates.
    pub stats: Option<Stats>,
}

pub struct Stats {
    pub bounds: Option<(Vec3, Vec3)>,
    pub triangles: usize,
}

impl App {
    /// Take the newest committed answer, and when the document changed under a standing preview, ask
    /// for another of the document with the preview lifted out.
    pub(crate) fn refresh_committed(&mut self, changed: bool) {
        let Some(tool) = self.simplify_tool.as_mut() else { return };
        if let Some(committed) = tool.committed.as_mut() {
            if let Some((result, _)) = committed.worker.poll() {
                committed.stats = Some(Stats { bounds: result.bounds, triangles: result.mesh.triangle_count() });
            }
        }
        if !changed || tool.shown.is_none() {
            return;
        }
        let lifted = self.lift_preview();
        if let Some(tool) = self.simplify_tool.as_mut() {
            let committed = tool.committed.get_or_insert_with(|| Committed { worker: EvalWorker::spawn(), stats: None });
            committed.worker.submit(&self.scene);
        }
        self.drop_preview_back(lifted);
    }

    /// The committed answer, while a preview stands and one has come back.
    fn committed_stats(&self) -> Option<&Stats> {
        let tool = self.simplify_tool.as_ref().filter(|tool| tool.shown.is_some())?;
        tool.committed.as_ref()?.stats.as_ref()
    }

    /// The model's bounds as committed, so the scene's size is not the preview's.
    pub(crate) fn committed_scene_bounds(&self) -> Option<(Vec3, Vec3)> {
        match self.committed_stats() {
            Some(stats) => stats.bounds,
            None => self.evaluated.bounds,
        }
    }

    /// The model's triangle count as committed, so the status bar does not report a result nobody has
    /// applied. Until the committed evaluation answers, the original's triangles stand in for the
    /// preview's, which is exact unless the mesh is in a boolean.
    pub(crate) fn committed_triangle_count(&self) -> usize {
        if let Some(stats) = self.committed_stats() {
            return stats.triangles;
        }
        let total = self.evaluated.mesh.triangle_count();
        let Some(tool) = self.simplify_tool.as_ref().filter(|tool| tool.shown.is_some()) else { return total };
        // Read from the evaluation, which until it catches up still holds the original.
        let Some(evaluated) = self.evaluated.node_meshes.get(&tool.target) else { return total };
        (total + tool.original.triangle_count()).saturating_sub(evaluated.triangle_count())
    }
}

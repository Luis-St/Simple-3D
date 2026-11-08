//! The model's numbers as committed while a preview stands in the document: the document evaluated a
//! second time with the original mesh put back, since what a boolean makes of the original cannot be
//! worked out from the preview's numbers.

use super::*;
use crate::worker::EvalWorker;
use simple3d_geom::Vec3;

/// The second evaluation, alive only while a preview stands, so its thread ends with it.
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
        // Gone with the preview, and with it the thread.
        if self.standing_preview().is_none() {
            self.preview_committed = None;
            return;
        }
        if let Some(committed) = self.preview_committed.as_mut() {
            if let Some((result, _)) = committed.worker.poll() {
                committed.stats = Some(Stats { bounds: result.bounds, triangles: result.mesh.triangle_count() });
            }
        }
        if !changed {
            return;
        }
        let lifted = self.lift_preview();
        let committed =
            self.preview_committed.get_or_insert_with(|| Committed { worker: EvalWorker::spawn(), stats: None });
        committed.worker.submit(&self.scene);
        self.drop_preview_back(lifted);
    }

    /// The committed answer, while a preview stands and one has come back.
    fn committed_stats(&self) -> Option<&Stats> {
        self.standing_preview()?;
        self.preview_committed.as_ref()?.stats.as_ref()
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
        let Some(standing) = self.standing_preview() else { return total };
        // Read from the evaluation, which until it catches up still holds the original.
        let Some(evaluated) = self.evaluated.node_meshes.get(&standing.target) else { return total };
        (total + standing.original.triangle_count()).saturating_sub(evaluated.triangle_count())
    }
}

impl App {
    /// The mesh `id` really holds: the original while a mesh tool previews on it, so the Properties panel
    /// does not report a result nobody has applied.
    pub(crate) fn committed_mesh(&self, id: NodeId) -> Option<Arc<MeshData>> {
        match self.mesh_tool_original() {
            Some((target, original)) if target == id => Some(original.clone()),
            _ => self.scene.get(id)?.mesh().cloned(),
        }
    }

    /// `id`'s world bounds as committed: the original mesh placed as the evaluation places the preview.
    pub(crate) fn committed_world_bounds(&self, id: NodeId) -> Option<(Vec3, Vec3)> {
        let previewed = self.standing_preview().filter(|standing| standing.target == id);
        let Some(standing) = previewed else { return self.evaluated.node_world_bounds.get(&id).copied() };
        let own = self.mesh_placement(id, standing.original)?;
        let mut points = standing.original.mesh.positions.iter().map(|&p| own.point(p));
        let first = points.next()?;
        Some(points.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }
}

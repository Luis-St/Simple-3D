//! Simplification on its own thread.

use super::job::PlannedJob;
use simple3d_core::mesh_data::MeshData;
use simple3d_geom::simplify::{Outcome, Simplify};
use std::sync::Arc;

/// A mesh being simplified (issue 106), off the UI thread because the tool is scrubbed; a run is
/// abandoned as soon as the next number is typed.
pub type SimplifyJob = PlannedJob<Simplify, Outcome>;

impl SimplifyJob {
    pub fn start(mesh: Arc<MeshData>, plan: Simplify) -> SimplifyJob {
        PlannedJob::spawn("simple3d-simplify", plan, move |plan, give_up| {
            simple3d_geom::simplify::simplify_until(&mesh.mesh, plan, give_up)
        })
    }
}

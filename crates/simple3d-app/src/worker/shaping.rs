//! Mesh functions on their own thread.

use super::job::PlannedJob;
use simple3d_core::mesh_data::MeshData;
use simple3d_geom::shaping::{Outcome, Shaping};
use std::sync::Arc;

/// A function running over a mesh (issue 81), off the UI thread because its numbers are scrubbed;
/// a run is abandoned as soon as the next number is typed.
pub type ShapeJob = PlannedJob<Shaping, Outcome>;

impl ShapeJob {
    pub fn start(mesh: Arc<MeshData>, plan: Shaping) -> ShapeJob {
        PlannedJob::spawn("simple3d-shape", plan, move |plan, give_up| {
            simple3d_geom::shaping::shape_until(&mesh.mesh, plan, give_up)
        })
    }
}

//! The reassembly running on its own thread.

use super::job::PlannedJob;
use simple3d_core::mesh_data::MeshData;
use simple3d_geom::reassemble::{Assembly, Reassemble};
use std::sync::Arc;

/// A mesh being taken apart (issue 108), off the interaction path since it takes a noticeable
/// fraction of a second, and abandonable since every scrubbed number asks again.
pub type ReassembleJob = PlannedJob<Reassemble, Assembly>;

impl ReassembleJob {
    pub fn start(mesh: Arc<MeshData>, plan: Reassemble) -> ReassembleJob {
        PlannedJob::spawn("simple3d-reassemble", plan, move |plan, give_up| {
            simple3d_geom::reassemble::reassemble_until(&mesh.mesh, plan, give_up)
        })
    }
}

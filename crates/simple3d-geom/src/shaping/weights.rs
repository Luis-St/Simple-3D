//! How strongly each vertex is moved: what the settings keep in place.

use super::*;
use crate::topology::Topology;

/// Each vertex's weight, 0 for one kept where it is to 1 for one moved fully: a hole's rim when
/// `keep_boundaries` is on, and the bottom `base` of the mesh when `keep_base` is, fading in over
/// the same height above it so no step forms where the kept part ends.
pub fn weights(mesh: &Mesh, topology: &Topology, plan: &Shaping) -> Vec<f64> {
    let mut out = vec![1.0; mesh.positions.len()];
    if plan.keep_base && plan.base > 0.0 {
        if let Some((lo, _)) = mesh.bounds() {
            for (weight, p) in out.iter_mut().zip(&mesh.positions) {
                let t = ((p.z - lo.z - plan.base) / plan.base).clamp(0.0, 1.0);
                *weight *= t * t * (3.0 - 2.0 * t);
            }
        }
    }
    if plan.keep_boundaries {
        for (weight, rim) in out.iter_mut().zip(topology.boundary_vertices()) {
            if rim {
                *weight = 0.0;
            }
        }
    }
    out
}

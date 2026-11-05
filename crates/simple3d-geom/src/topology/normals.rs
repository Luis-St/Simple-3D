//! A normal per vertex, for moving a surface along itself and for smooth shading.

use super::*;
use crate::vec3::Vec3;

/// Each vertex's outward unit normal: its triangles' normals weighted by the angle each has at the
/// vertex. `Vec3::ZERO` for a vertex with no triangle of any area.
///
/// Weighted by angle rather than area or count, so how a flat face is triangulated does not tilt
/// it (Thürmer and Wüthrich): a box corner points along its diagonal whichever way its faces are
/// split. Smooth across every crease, so a box corner's normal leans out of all three faces.
pub fn vertex_normals(mesh: &Mesh) -> Vec<Vec3> {
    let mut sums = vec![Vec3::ZERO; mesh.positions.len()];
    for tri in &mesh.indices {
        let corners = mesh.corners(*tri);
        let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalized();
        if normal == Vec3::ZERO {
            continue;
        }
        for k in 0..3 {
            let to_next = corners[(k + 1) % 3] - corners[k];
            let to_prev = corners[(k + 2) % 3] - corners[k];
            // atan2 rather than acos of a dot product, which loses precision near 0 and 180 degrees.
            let angle = to_next.cross(to_prev).length().atan2(to_next.dot(to_prev));
            sums[tri[k] as usize] = sums[tri[k] as usize] + normal * angle;
        }
    }
    sums.into_iter().map(Vec3::normalized).collect()
}

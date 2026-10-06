//! Whether a point is inside a closed mesh.

use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Whether `p` is inside the closed `mesh`, by the parity of its crossings along a ray. The ray's
/// direction is skewed off every axis, so it does not run along the edges of axis-aligned shapes.
pub fn contains_point(mesh: &Mesh, p: Vec3) -> bool {
    let dir = Vec3::new(0.5377, 0.6261, 0.5646).normalized();
    let mut crossings = 0;
    for tri in &mesh.indices {
        if let Some(t) = crate::ray::line_triangle(p, dir, mesh.corners(*tri), 0.0) {
            if t > 1e-9 {
                crossings += 1;
            }
        }
    }
    crossings % 2 == 1
}

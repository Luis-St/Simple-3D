//! The surface a ray meets first, passing through some objects' own faces.

use super::*;
use simple3d_geom::{Mesh, Vec3};
use std::sync::Arc;

/// Where the ray first meets a triangle of `mesh` that `skip` does not pass through (by its source),
/// which triangle, and its normal turned towards the ray. Used to put things onto the model
/// (issues 117 and 118), where the thing being put down must not land on itself.
pub(crate) fn surface_hit(
    mesh: &Arc<Mesh>,
    origin: Vec3,
    dir: Vec3,
    skip: impl Fn(u32) -> bool,
) -> Option<(Vec3, usize, Vec3)> {
    // A few objects in a row can be passed through; past that the pointer is lost in a crowd.
    const MOST_PASSED: usize = 64;
    let mut from = origin;
    for _ in 0..MOST_PASSED {
        let (t, triangle) = ray_mesh_triangle(mesh, from, dir)?;
        let at = from + dir * t;
        if skip(mesh.source(triangle)) {
            from = at + dir * 1e-6;
            continue;
        }
        let normal = mesh.triangle_normal(mesh.indices[triangle]).normalized();
        let normal = if normal.dot(dir) > 0.0 { normal * -1.0 } else { normal };
        return Some((at, triangle, normal));
    }
    None
}

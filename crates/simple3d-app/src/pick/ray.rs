//! A ray against a triangle, a mesh and a box.

use super::bvh::{tree_for, MIN_TRIANGLES};
use simple3d_geom::{Mesh, Vec3};
use std::sync::Arc;

/// The distance at which the ray enters the triangle, if it does: either facing, so a click inside
/// a solid still hits.
pub fn ray_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f64> {
    simple3d_geom::ray::line_triangle(origin, dir, [a, b, c], 1e-9).filter(|&t| t >= 1e-6)
}

/// The nearest hit on a mesh. Large meshes use a cached BVH, since the measure tool casts every
/// frame; the answer matches walking every triangle ([`Bvh::nearest`]).
///
/// [`Bvh::nearest`]: super::bvh::Bvh::nearest
pub fn ray_mesh(mesh: &Arc<Mesh>, origin: Vec3, dir: Vec3) -> Option<f64> {
    if mesh.indices.len() < MIN_TRIANGLES {
        return ray_mesh_linear(mesh, origin, dir);
    }
    tree_for(mesh).nearest(mesh, origin, dir)
}

/// The nearest hit with the index of the triangle hit, for tools that need the face (issue 73).
pub fn ray_mesh_triangle(mesh: &Arc<Mesh>, origin: Vec3, dir: Vec3) -> Option<(f64, usize)> {
    if mesh.indices.len() < MIN_TRIANGLES {
        return ray_mesh_linear_triangle(mesh, origin, dir);
    }
    tree_for(mesh).nearest_triangle(mesh, origin, dir)
}

/// The same, by walking every triangle.
pub(crate) fn ray_mesh_linear(mesh: &Mesh, origin: Vec3, dir: Vec3) -> Option<f64> {
    ray_mesh_linear_triangle(mesh, origin, dir).map(|(t, _)| t)
}

fn ray_mesh_linear_triangle(mesh: &Mesh, origin: Vec3, dir: Vec3) -> Option<(f64, usize)> {
    // A bounding-box reject first, keeping clicks instant with many primitives.
    let (lo, hi) = mesh.bounds()?;
    ray_box(origin, dir, lo, hi)?;
    let mut nearest: Option<(f64, usize)> = None;
    for (index, tri) in mesh.indices.iter().enumerate() {
        let [a, b, c] = mesh.corners(*tri);
        if let Some(t) = ray_triangle(origin, dir, a, b, c) {
            if nearest.is_none_or(|(best, _)| t < best) {
                nearest = Some((t, index));
            }
        }
    }
    nearest
}

/// Slab test. Returns the entry distance, or `None` if the ray misses.
pub fn ray_box(origin: Vec3, dir: Vec3, lo: Vec3, hi: Vec3) -> Option<f64> {
    simple3d_geom::ray::ray_box(origin, dir, lo, hi, f64::INFINITY)
}

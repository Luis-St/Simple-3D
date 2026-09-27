//! A ray against a triangle, a mesh and a box.

use super::bvh::{tree_for, MIN_TRIANGLES};
use simple3d_geom::{Mesh, Vec3};
use std::sync::Arc;

/// The distance at which the ray enters the triangle, if it does: Moeller-Trumbore, either facing,
/// so a click inside a solid still hits.
pub fn ray_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(h) * inv;
    if u < -1e-9 || u > 1.0 + 1e-9 {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    let t = e2.dot(q) * inv;
    if t < 1e-6 {
        return None;
    }
    Some(t)
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

/// The same, by walking every triangle.
pub(crate) fn ray_mesh_linear(mesh: &Mesh, origin: Vec3, dir: Vec3) -> Option<f64> {
    // A bounding-box reject first, keeping clicks instant with many primitives.
    let (lo, hi) = mesh.bounds()?;
    ray_box(origin, dir, lo, hi)?;
    let mut nearest: Option<f64> = None;
    for tri in &mesh.indices {
        let hit = ray_triangle(
            origin,
            dir,
            mesh.positions[tri[0] as usize],
            mesh.positions[tri[1] as usize],
            mesh.positions[tri[2] as usize],
        );
        if let Some(t) = hit {
            if nearest.is_none_or(|best| t < best) {
                nearest = Some(t);
            }
        }
    }
    nearest
}

/// Slab test. Returns the entry distance, or `None` if the ray misses.
pub fn ray_box(origin: Vec3, dir: Vec3, lo: Vec3, hi: Vec3) -> Option<f64> {
    let mut t_min = f64::NEG_INFINITY;
    let mut t_max = f64::INFINITY;
    for axis in 0..3 {
        let (o, d) = match axis {
            0 => (origin.x, dir.x),
            1 => (origin.y, dir.y),
            _ => (origin.z, dir.z),
        };
        let (l, h) = match axis {
            0 => (lo.x, hi.x),
            1 => (lo.y, hi.y),
            _ => (lo.z, hi.z),
        };
        if d.abs() < 1e-12 {
            if o < l || o > h {
                return None;
            }
            continue;
        }
        let (mut near, mut far) = ((l - o) / d, (h - o) / d);
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        t_min = t_min.max(near);
        t_max = t_max.min(far);
        if t_min > t_max {
            return None;
        }
    }
    if t_max < 0.0 {
        return None;
    }
    Some(t_min.max(0.0))
}

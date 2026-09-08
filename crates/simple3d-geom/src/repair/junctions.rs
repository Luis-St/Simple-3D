//! T-junctions: a vertex lying on another triangle's edge, and the split
//! that makes the two agree.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// Split every triangle edge that has another vertex of the mesh lying on its
/// interior, so each undirected edge ends up shared by exactly two triangles.
///
/// Every triangle is dealt with **once**, from the vertices the mesh had when
/// the pass started. The obvious implementation instead splits a triangle in
/// two and pushes both halves back into the queue, and that is what shipped
/// first; it is unbounded. A split introduces an *internal* edge from the split
/// point to the opposite corner, that new edge is examined in its turn, and any
/// vertex within a micron of it -- a vertex that was never on the surface's
/// boundary and needs no split at all -- sets off another. On a spherical cap
/// unioned with a plate the cascade turned 50,854 triangles into 2,679,216 and
/// ran out of its own budget, leaving the mesh non-manifold: a boolean at 176
/// segments produced a quarter of a gigabyte of garbage where 25,000 triangles
/// describe the solid.
///
/// Collecting each edge's on-edge vertices up front and triangulating the
/// resulting polygon in one go cannot cascade: the internal edges it creates
/// are never looked at. They need not be. A T-junction is a vertex on the
/// *boundary* between two faces, and the boundary is exactly what the up-front
/// collection sees.
pub(crate) fn split_t_junctions(mesh: Mesh, tol: f64) -> Mesh {
    if mesh.indices.is_empty() {
        return mesh;
    }
    let (lo, hi) = mesh.bounds().unwrap();
    let extent = (hi - lo).x.max((hi - lo).y).max((hi - lo).z);
    let size = (extent / 48.0).max(tol * 16.0);

    let mut grid: HashMap<Cell, Vec<u32>> = HashMap::new();
    for (i, &p) in mesh.positions.iter().enumerate() {
        grid.entry(cell_of(p, size)).or_default().push(i as u32);
    }

    let mut positions = mesh.positions.clone();
    let pos = &mesh.positions;
    let mut indices: Vec<[u32; 3]> = Vec::with_capacity(mesh.indices.len());
    let mut tags: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    // The triangle's boundary with the on-edge vertices spliced into it, reused
    // across triangles rather than reallocated for each.
    let mut loop_: Vec<u32> = Vec::new();
    let mut on_edge: [Vec<OnEdge>; 3] = [Vec::new(), Vec::new(), Vec::new()];

    for (i, tri) in mesh.indices.iter().enumerate() {
        let tag = mesh.tag(i);
        for (e, out) in on_edge.iter_mut().enumerate() {
            on_edge_vertices(pos, &grid, size, tol, tri, e, out);
        }
        loop_.clear();
        for e in 0..3 {
            loop_.push(tri[e]);
            loop_.extend(on_edge[e].iter().map(|v| v.vertex));
        }
        if loop_.len() == 3 {
            indices.push(*tri);
            tags.push(tag);
            continue;
        }
        let mut deduped = loop_.clone();
        deduped.sort_unstable();
        deduped.dedup();
        if deduped.len() == loop_.len() {
            fan_from_centre(pos, tri, &loop_, tag, &mut positions, &mut indices, &mut tags);
        } else {
            for piece in split_pinched_loops(&loop_) {
                fan_loop_from_own_centre(pos, &piece, tag, &mut positions, &mut indices, &mut tags);
            }
        }
    }

    Mesh { positions, indices, tags }
}

/// A mesh vertex found lying on one edge of a triangle: how far along that edge
/// it sits, and which vertex it is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OnEdge {
    pub(super) along: f64,
    pub(super) vertex: u32,
}

pub(crate) fn on_edge_vertices(
    pos: &[Vec3],
    grid: &HashMap<Cell, Vec<u32>>,
    size: f64,
    tol: f64,
    tri: &[u32; 3],
    e: usize,
    out: &mut Vec<OnEdge>,
) {
    out.clear();
    let (ia, ib) = (tri[e], tri[(e + 1) % 3]);
    let (pa, pb) = (pos[ia as usize], pos[ib as usize]);
    let ab = pb - pa;
    let len2 = ab.dot(ab);
    if len2 <= tol * tol {
        return;
    }
    let margin = tol / len2.sqrt();

    let lo = pa.min(pb) - Vec3::splat(tol);
    let hi = pa.max(pb) + Vec3::splat(tol);
    let (c0, c1) = (cell_of(lo, size), cell_of(hi, size));
    for cx in c0.0..=c1.0 {
        for cy in c0.1..=c1.1 {
            for cz in c0.2..=c1.2 {
                let Some(list) = grid.get(&(cx, cy, cz)) else { continue };
                for &v in list {
                    if v == tri[0] || v == tri[1] || v == tri[2] {
                        continue;
                    }
                    let d = pos[v as usize] - pa;
                    let s = d.dot(ab) / len2;
                    if s <= margin || s >= 1.0 - margin {
                        continue;
                    }
                    if (d - ab * s).length() > tol {
                        continue;
                    }
                    out.push(OnEdge { along: s, vertex: v });
                }
            }
        }
    }
    out.sort_by(|a, b| a.along.total_cmp(&b.along));
    // The same physical point can be present twice over -- the grid is searched
    // by cell, and a vertex sitting exactly on a cell boundary is listed in
    // both. Two boundary vertices at the same place would make a zero-length
    // edge, and the ear clipper below would have to cope with it.
    out.dedup_by(|a, b| (a.along - b.along).abs() <= f64::EPSILON || a.vertex == b.vertex);
}

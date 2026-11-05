//! Normals per vertex and per triangle corner, for moving a surface along itself and for smooth
//! shading.

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
    for (tri, (normal, angles)) in mesh.indices.iter().zip(faces_of(mesh)) {
        for k in 0..3 {
            sums[tri[k] as usize] = sums[tri[k] as usize] + normal * angles[k];
        }
    }
    sums.into_iter().map(Vec3::normalized).collect()
}

/// Each triangle's normal at its three corners, smooth across the edges where the surface turns
/// by less than `crease_deg` and split where it turns by more: a box stays sharp, a finely
/// faceted cylinder shades round.
///
/// A corner's normal is the angle-weighted mean (as [`vertex_normals`]) of the triangles around
/// its vertex that are reached without crossing a crease. Only triangles `smooth` marks take part;
/// the rest, and degenerate ones, get `Vec3::ZERO` corners, and an edge to one counts as a crease.
/// So does a boundary or a junction: neither has one surface on both sides.
pub fn corner_normals(
    mesh: &Mesh,
    edges: &EdgeTable,
    faces: &Rows<u32>,
    crease_deg: f64,
    smooth: &[bool],
) -> Vec<[Vec3; 3]> {
    let cos_limit = crease_deg.to_radians().cos();
    let shape = faces_of(mesh);
    let usable = |face: u32| smooth[face as usize] && shape[face as usize].0 != Vec3::ZERO;
    let mut out = vec![[Vec3::ZERO; 3]; mesh.indices.len()];
    // Reused per vertex: its usable triangles, and a union-find over their positions in that list.
    let (mut ring, mut parent, mut sums) = (Vec::new(), Vec::new(), Vec::new());
    for v in 0..faces.len() {
        ring.clear();
        ring.extend(faces.row(v).iter().copied().filter(|&face| usable(face)));
        if ring.is_empty() {
            continue;
        }
        parent.clear();
        parent.extend(0..ring.len());
        for (i, &face) in ring.iter().enumerate() {
            // The two edges of `face` that meet at `v`.
            for &other in mesh.indices[face as usize].iter().filter(|&&corner| corner as usize != v) {
                let Some(edge) = edges.find(v as u32, other) else { continue };
                let &[a, b] = edges.faces(edge) else { continue };
                let across = if a == face { b } else { a };
                let Some(j) = ring.iter().position(|&f| f == across) else { continue };
                if shape[face as usize].0.dot(shape[across as usize].0) >= cos_limit {
                    let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                    parent[ri] = rj;
                }
            }
        }
        sums.clear();
        sums.resize(ring.len(), Vec3::ZERO);
        for (i, &face) in ring.iter().enumerate() {
            let (normal, angles) = shape[face as usize];
            let at = corner_of(mesh, face, v);
            let r = root(&mut parent, i);
            sums[r] = sums[r] + normal * angles[at];
        }
        for (i, &face) in ring.iter().enumerate() {
            let r = root(&mut parent, i);
            out[face as usize][corner_of(mesh, face, v)] = sums[r].normalized();
        }
    }
    out
}

/// Each triangle's unit normal and the angle at each corner; a zero normal and zero angles for a
/// degenerate one.
fn faces_of(mesh: &Mesh) -> Vec<(Vec3, [f64; 3])> {
    mesh.indices
        .iter()
        .map(|tri| {
            let corners = mesh.corners(*tri);
            let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalized();
            if normal == Vec3::ZERO {
                return (Vec3::ZERO, [0.0; 3]);
            }
            let angles = std::array::from_fn(|k| {
                let to_next = corners[(k + 1) % 3] - corners[k];
                let to_prev = corners[(k + 2) % 3] - corners[k];
                // atan2 rather than acos of a dot product, which loses precision near 0 and 180 degrees.
                to_next.cross(to_prev).length().atan2(to_next.dot(to_prev))
            });
            (normal, angles)
        })
        .collect()
}

/// Which corner of triangle `face` vertex `v` is.
fn corner_of(mesh: &Mesh, face: u32, v: usize) -> usize {
    mesh.indices[face as usize].iter().position(|&corner| corner as usize == v).expect("a triangle of the vertex")
}

fn root(parent: &mut [usize], mut of: usize) -> usize {
    while parent[of] != of {
        parent[of] = parent[parent[of]];
        of = parent[of];
    }
    of
}

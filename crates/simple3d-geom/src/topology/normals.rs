//! Normals per vertex and per triangle corner, for moving a surface along itself and for smooth
//! shading, for the whole mesh or again for the few vertices an edit moved (the brush, issue 127).

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

/// [`vertex_normals`] again for `vertices` only, into `out`, from each one's triangles (`faces`,
/// [`vertex_faces`]). What an edit needs after moving some vertices: their normals and their
/// neighbours' change, nothing else does.
pub fn vertex_normals_at(mesh: &Mesh, faces: &Rows<u32>, vertices: &[u32], out: &mut [Vec3]) {
    for &v in vertices {
        let sum = faces.row(v as usize).iter().fold(Vec3::ZERO, |sum, &face| {
            let (normal, angles) = face_shape(mesh, face);
            sum + normal * angles[corner_of(mesh, face, v as usize)]
        });
        out[v as usize] = sum.normalized();
    }
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
    let shape = faces_of(mesh);
    let mut out = vec![[Vec3::ZERO; 3]; mesh.indices.len()];
    let mut corners = Corners::new(mesh, edges, faces, crease_deg, smooth);
    for v in 0..faces.len() {
        corners.at(v, |face| shape[face as usize], &mut out);
    }
    out
}

/// [`corner_normals`] again at `vertices` only, into `out`: every corner of their triangles that
/// sits on one of them. After moving some vertices, pass them and their neighbours, since a
/// moved vertex turns the triangles around each neighbour too.
pub fn corner_normals_at(
    mesh: &Mesh,
    edges: &EdgeTable,
    faces: &Rows<u32>,
    crease_deg: f64,
    smooth: &[bool],
    vertices: &[u32],
    out: &mut [[Vec3; 3]],
) {
    let mut corners = Corners::new(mesh, edges, faces, crease_deg, smooth);
    for &v in vertices {
        corners.at(v as usize, |face| face_shape(mesh, face), out);
    }
}

/// What working out one vertex's corners needs, with the buffers reused from vertex to vertex.
struct Corners<'a> {
    mesh: &'a Mesh,
    edges: &'a EdgeTable,
    faces: &'a Rows<u32>,
    cos_limit: f64,
    smooth: &'a [bool],
    /// The vertex's usable triangles, a union-find over their places in that list, and the sum of
    /// each group's normals.
    ring: Vec<(u32, (Vec3, [f64; 3]))>,
    parent: Vec<usize>,
    sums: Vec<Vec3>,
}

impl<'a> Corners<'a> {
    fn new(mesh: &'a Mesh, edges: &'a EdgeTable, faces: &'a Rows<u32>, crease_deg: f64, smooth: &'a [bool]) -> Self {
        let cos_limit = crease_deg.to_radians().cos();
        Corners { mesh, edges, faces, cos_limit, smooth, ring: Vec::new(), parent: Vec::new(), sums: Vec::new() }
    }

    /// The corners at vertex `v`, with each triangle's shape from `shape`.
    fn at(&mut self, v: usize, shape: impl Fn(u32) -> (Vec3, [f64; 3]), out: &mut [[Vec3; 3]]) {
        let mesh = self.mesh;
        self.ring.clear();
        for &face in self.faces.row(v) {
            if !self.smooth[face as usize] {
                continue;
            }
            let shaped = shape(face);
            if shaped.0 == Vec3::ZERO {
                // Degenerate: no corner normal, and no part in its neighbours'.
                out[face as usize][corner_of(mesh, face, v)] = Vec3::ZERO;
                continue;
            }
            self.ring.push((face, shaped));
        }
        if self.ring.is_empty() {
            return;
        }
        self.parent.clear();
        self.parent.extend(0..self.ring.len());
        for i in 0..self.ring.len() {
            let (face, (normal, _)) = self.ring[i];
            // The two edges of `face` that meet at `v`.
            for &other in mesh.indices[face as usize].iter().filter(|&&corner| corner as usize != v) {
                let Some(edge) = self.edges.find(v as u32, other) else { continue };
                let &[a, b] = self.edges.faces(edge) else { continue };
                let across = if a == face { b } else { a };
                let Some(j) = self.ring.iter().position(|&(f, _)| f == across) else { continue };
                if normal.dot(self.ring[j].1 .0) >= self.cos_limit {
                    let (ri, rj) = (root(&mut self.parent, i), root(&mut self.parent, j));
                    self.parent[ri] = rj;
                }
            }
        }
        self.sums.clear();
        self.sums.resize(self.ring.len(), Vec3::ZERO);
        for i in 0..self.ring.len() {
            let (face, (normal, angles)) = self.ring[i];
            let r = root(&mut self.parent, i);
            self.sums[r] = self.sums[r] + normal * angles[corner_of(mesh, face, v)];
        }
        for i in 0..self.ring.len() {
            let face = self.ring[i].0;
            let r = root(&mut self.parent, i);
            out[face as usize][corner_of(mesh, face, v)] = self.sums[r].normalized();
        }
    }
}

/// Each triangle's unit normal and the angle at each corner ([`face_shape`]).
fn faces_of(mesh: &Mesh) -> Vec<(Vec3, [f64; 3])> {
    (0..mesh.indices.len() as u32).map(|face| face_shape(mesh, face)).collect()
}

/// A triangle's unit normal and the angle at each corner; a zero normal and zero angles for a
/// degenerate one.
fn face_shape(mesh: &Mesh, face: u32) -> (Vec3, [f64; 3]) {
    let corners = mesh.corners(mesh.indices[face as usize]);
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

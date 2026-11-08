//! How a welded mesh's triangles connect: its edges with the triangles meeting along them, each
//! vertex's triangles and neighbours, its connected bodies, and its normals per vertex and per
//! triangle corner.
//!
//! One place for what the renderer, the mesh functions (issue 81) and the brush (issue 127) all
//! read, instead of each building its own. Every table is a set of [`Rows`], filled by counting
//! first: a hash map of edges took most of a second on a 1.5M-triangle import.
//!
//! All of it expects a welded mesh ([`Mesh::weld`]). Unwelded, every triangle is its own body and
//! every edge a boundary.

mod rows;
pub use rows::{Rows, RowsBuilder};
mod edges;
pub use edges::EdgeTable;
mod bodies;
pub use bodies::connected_bodies;
mod normals;
pub use normals::{corner_normals, corner_normals_at, vertex_normals, vertex_normals_at};
#[cfg(test)]
mod tests;

use crate::mesh::Mesh;

/// The triangles touching each vertex, in ascending order.
pub fn vertex_faces(mesh: &Mesh) -> Rows<u32> {
    let mut rows = RowsBuilder::new(mesh.positions.len());
    for tri in &mesh.indices {
        for &v in tri {
            rows.count(v as usize);
        }
    }
    for (face, tri) in mesh.indices.iter().enumerate() {
        for &v in tri {
            rows.push(v as usize, face as u32);
        }
    }
    rows.finish()
}

/// A mesh's connectivity, built once and read by the edits that move its vertices.
#[derive(Clone, Debug)]
pub struct Topology {
    pub edges: EdgeTable,
    /// The triangles touching each vertex ([`vertex_faces`]).
    pub faces: Rows<u32>,
    /// The vertices sharing an edge with each vertex ([`EdgeTable::neighbours`]).
    pub neighbours: Rows<u32>,
}

impl Topology {
    pub fn of(mesh: &Mesh) -> Topology {
        let edges = EdgeTable::of(mesh);
        let neighbours = edges.neighbours(mesh.positions.len());
        Topology { edges, faces: vertex_faces(mesh), neighbours }
    }

    /// Whether each vertex lies on a hole's rim: an edge of it has only one triangle. Edits keep
    /// these where they are unless asked otherwise, as Simplify does.
    pub fn boundary_vertices(&self) -> Vec<bool> {
        let mut out = vec![false; self.neighbours.len()];
        for (ends, faces) in self.edges.iter() {
            if faces.len() == 1 {
                out[ends[0] as usize] = true;
                out[ends[1] as usize] = true;
            }
        }
        out
    }
}

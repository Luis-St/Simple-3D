//! Geometry a node owns outright, and how it is written to the project file.
//!
//! Everything else in the tree is a *recipe*: a box is three numbers, a
//! boolean is its operands, and the triangles are made again on every load. A
//! mesh body is the exception -- it is what a shape becomes when it is converted
//! (issue 80), and after that there is no recipe left to keep, only the surface.
//!
//! ## Why it is not written as arrays of numbers
//!
//! The project file is pretty-printed JSON, one value per line, so that a
//! changed dimension is a one-line diff. `serde_json` puts every element of an
//! array on its own line, and a converted assembly is easily a hundred thousand
//! triangles: written as numbers that is over a million lines, and a file no
//! editor will open. So the arrays go in as base64 of their little-endian bytes
//! -- one line each -- beside a plainly readable triangle count for a human
//! reading the file.
//!
//! Positions are stored as `f32`. A mesh body is always the *result* of a
//! tessellation rather than a dimension anybody typed, and 24 bits of mantissa
//! resolves a hundredth of a millimetre out to a metre, which is finer than
//! anything downstream of it. Doubles would halve the tolerance and double the
//! file for geometry that is already an approximation of a surface.

mod blob;
pub use blob::MeshBlob;
pub(crate) use blob::*;
mod base64;
pub(crate) use base64::*;
#[cfg(test)]
mod tests;

use simple3d_geom::{Mesh, Vec3};

/// A mesh a node owns. Immutable: a boolean builds a new one, so a
/// stored mesh can be shared behind an `Arc` and a scene snapshot for undo costs
/// a pointer rather than a copy of every triangle.
#[derive(Clone, Debug)]
pub struct MeshData {
    pub mesh: Mesh,
}

/// Two stored meshes are the same when they describe the same surface. Written
/// out rather than derived because `Mesh` has no equality of its own -- meshes
/// are compared by what they *are* nowhere else in the application, and giving
/// the geometry crate a blanket `PartialEq` would invite exactly the
/// vertex-by-vertex comparison this one exists to keep rare.
impl PartialEq for MeshData {
    fn eq(&self, other: &Self) -> bool {
        self.mesh.indices == other.mesh.indices
            && self.mesh.tags == other.mesh.tags
            && self.mesh.positions == other.mesh.positions
    }
}

impl MeshData {
    pub fn new(mesh: Mesh) -> MeshData {
        // Welded on the way in: a converted mesh is stored once and read many
        // times, so the compact form is the one worth keeping.
        MeshData { mesh: mesh.weld() }
    }

    pub fn triangle_count(&self) -> usize {
        self.mesh.triangle_count()
    }

    pub fn to_blob(&self) -> MeshBlob {
        let mut positions = Vec::with_capacity(self.mesh.positions.len() * 12);
        for p in &self.mesh.positions {
            positions.extend_from_slice(&(p.x as f32).to_le_bytes());
            positions.extend_from_slice(&(p.y as f32).to_le_bytes());
            positions.extend_from_slice(&(p.z as f32).to_le_bytes());
        }
        let mut indices = Vec::with_capacity(self.mesh.indices.len() * 12);
        for t in &self.mesh.indices {
            for v in t {
                indices.extend_from_slice(&v.to_le_bytes());
            }
        }
        MeshBlob {
            triangles: self.mesh.indices.len(),
            vertices: self.mesh.positions.len(),
            positions: encode(&positions),
            indices: encode(&indices),
            tags: encode_tags(&self.mesh.tags, self.mesh.indices.len()),
        }
    }

    /// Read a blob back. `None` when it does not describe a mesh -- a truncated
    /// array, an index past the end of the vertices -- so a damaged file is
    /// refused with a message rather than loaded as a half-mesh.
    pub fn from_blob(blob: &MeshBlob) -> Option<MeshData> {
        let position_bytes = decode(&blob.positions)?;
        if position_bytes.len() % 12 != 0 {
            return None;
        }
        let positions: Vec<Vec3> = position_bytes
            .chunks_exact(12)
            .map(|c| {
                let f = |at: usize| f32::from_le_bytes([c[at], c[at + 1], c[at + 2], c[at + 3]]) as f64;
                Vec3::new(f(0), f(4), f(8))
            })
            .collect();
        let index_bytes = decode(&blob.indices)?;
        if index_bytes.len() % 12 != 0 {
            return None;
        }
        let mut indices = Vec::with_capacity(index_bytes.len() / 12);
        for c in index_bytes.chunks_exact(12) {
            let v = |at: usize| u32::from_le_bytes([c[at], c[at + 1], c[at + 2], c[at + 3]]);
            let tri = [v(0), v(4), v(8)];
            if tri.iter().any(|&i| i as usize >= positions.len()) {
                return None;
            }
            indices.push(tri);
        }
        let tags = decode_tags(&blob.tags, indices.len())?;
        Some(MeshData { mesh: Mesh { positions, indices, tags } })
    }
}

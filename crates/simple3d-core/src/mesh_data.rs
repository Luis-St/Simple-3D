//! Geometry a node owns outright, and how it is written to the project file.
//!
//! Unlike recipes, a converted mesh (issue 80) keeps only its surface. Arrays are written as
//! base64 of little-endian bytes, one line each, since pretty-printed JSON would put every number
//! on its own line. Positions are `f32`: enough for 0.01 mm at a metre, for geometry that is
//! already a tessellation.

mod blob;
pub use blob::MeshBlob;
pub(crate) use blob::*;
mod base64;
pub(crate) use base64::*;
#[cfg(test)]
mod tests;

use simple3d_geom::{Mesh, Vec3};

/// A mesh a node owns. Immutable, so it can be shared behind an `Arc` and undo snapshots stay cheap.
#[derive(Clone, Debug)]
pub struct MeshData {
    pub mesh: Mesh,
}

/// Equal when describing the same surface. Written by hand since `Mesh` deliberately has no
/// `PartialEq`, keeping vertex-by-vertex comparison rare.
impl PartialEq for MeshData {
    fn eq(&self, other: &Self) -> bool {
        self.mesh.indices == other.mesh.indices
            && self.mesh.tags == other.mesh.tags
            && self.mesh.positions == other.mesh.positions
    }
}

impl MeshData {
    pub fn new(mesh: Mesh) -> MeshData {
        // Welded on the way in: stored once, read many times.
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

    /// Read a blob back; `None` if it is not a valid mesh, so a damaged file is refused.
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
        Some(MeshData { mesh: Mesh { positions, indices, tags, sources: Vec::new() } })
    }
}

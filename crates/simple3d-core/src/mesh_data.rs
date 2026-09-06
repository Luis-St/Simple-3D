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

use serde::{Deserialize, Serialize};
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

/// The stored form. `triangles` and `vertices` are for the person reading the
/// file; the loader trusts the arrays and checks them against each other.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshBlob {
    pub triangles: usize,
    pub vertices: usize,
    pub positions: String,
    pub indices: String,
    /// Run-length encoded, and absent entirely from a mesh nobody painted --
    /// which is most of them.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tags: String,
}

/// Per-triangle colour tags as runs of `count:tag`, which for a converted solid
/// is a handful of pairs rather than one number per triangle. An empty string
/// means every triangle is untagged.
fn encode_tags(tags: &[u32], triangles: usize) -> String {
    if tags.iter().all(|&t| t == 0) {
        return String::new();
    }
    let mut out = String::new();
    let mut run_tag = tags.first().copied().unwrap_or(0);
    let mut run = 0usize;
    for index in 0..triangles {
        let tag = tags.get(index).copied().unwrap_or(0);
        if tag == run_tag {
            run += 1;
            continue;
        }
        out.push_str(&format!("{run}:{run_tag} "));
        run_tag = tag;
        run = 1;
    }
    if run > 0 {
        out.push_str(&format!("{run}:{run_tag}"));
    }
    out.trim_end().to_string()
}

fn decode_tags(text: &str, triangles: usize) -> Option<Vec<u32>> {
    if text.is_empty() {
        return Some(vec![0; triangles]);
    }
    let mut out = Vec::with_capacity(triangles);
    for run in text.split_whitespace() {
        let (count, tag) = run.split_once(':')?;
        let count: usize = count.parse().ok()?;
        let tag: u32 = tag.parse().ok()?;
        if out.len() + count > triangles {
            return None;
        }
        out.extend(std::iter::repeat_n(tag, count));
    }
    // A short run list is not an error: it means the rest is untagged.
    out.resize(triangles, 0);
    Some(out)
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64, written here rather than taken as a dependency: it is
/// twenty lines, and the shipped binary is deliberately self-contained.
fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut accumulator: u32 = 0;
    let mut bits = 0u32;
    for c in text.bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let value = ALPHABET.iter().position(|&a| a == c)? as u32;
        accumulator = (accumulator << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple3d_geom::primitives;

    #[test]
    fn a_mesh_survives_the_file_it_is_written_to() {
        let mut mesh = primitives::ellipsoid_mesh(40.0, 30.0, 20.0, 24);
        mesh.set_tag(simple3d_geom::colour_tag([0x2E, 0x9A, 0xFF]));
        let data = MeshData::new(mesh);
        let blob = data.to_blob();
        let back = MeshData::from_blob(&blob).expect("it should read back");
        assert_eq!(back.mesh.indices, data.mesh.indices);
        assert_eq!(back.mesh.tags, data.mesh.tags);
        assert_eq!(back.mesh.positions.len(), data.mesh.positions.len());
        for (a, b) in back.mesh.positions.iter().zip(&data.mesh.positions) {
            // Stored as f32: exact to the last place a millimetre-scale model
            // has, which is what the doc comment claims.
            assert!((*a - *b).length() < 1e-3, "{a:?} came back as {b:?}");
        }
    }

    #[test]
    fn several_colours_survive_as_runs() {
        let mut mesh = primitives::box_mesh(10.0, 10.0, 10.0);
        mesh.set_tag(7);
        let half = mesh.indices.len() / 2;
        for tag in mesh.tags.iter_mut().take(half) {
            *tag = 9;
        }
        let data = MeshData::new(mesh.clone());
        let blob = data.to_blob();
        assert!(blob.tags.contains(':'), "the runs were not written: {:?}", blob.tags);
        let back = MeshData::from_blob(&blob).unwrap();
        assert_eq!(back.mesh.tags, data.mesh.tags);
    }

    #[test]
    fn an_unpainted_mesh_writes_no_tags_at_all() {
        // So a mesh body diffs as its geometry and nothing else.
        let blob = MeshData::new(primitives::box_mesh(5.0, 5.0, 5.0)).to_blob();
        assert!(blob.tags.is_empty());
        assert_eq!(MeshData::from_blob(&blob).unwrap().mesh.tags.len(), blob.triangles);
    }

    #[test]
    fn a_damaged_blob_is_refused_rather_than_half_read() {
        let good = MeshData::new(primitives::box_mesh(10.0, 10.0, 10.0)).to_blob();

        let mut truncated = good.clone();
        truncated.positions.truncate(good.positions.len() - 6);
        assert!(MeshData::from_blob(&truncated).is_none(), "a truncated vertex array was accepted");

        let mut wrong = good.clone();
        // An index array of a length that is not a whole number of triangles.
        wrong.indices.truncate(4);
        assert!(MeshData::from_blob(&wrong).is_none());

        let mut past_the_end = good.clone();
        past_the_end.positions = encode(&[0u8; 12]);
        assert!(MeshData::from_blob(&past_the_end).is_none(), "an index past the last vertex was accepted");

        let mut nonsense = good;
        nonsense.positions = "not base64 at all !!!".into();
        assert!(MeshData::from_blob(&nonsense).is_none());
    }

    #[test]
    fn base64_round_trips_every_length() {
        for len in 0..40usize {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 37 % 251) as u8).collect();
            assert_eq!(decode(&encode(&bytes)).unwrap(), bytes, "length {len}");
        }
        // And it is the standard encoding, not a private one.
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
        assert_eq!(encode(b"M"), "TQ==");
    }

    #[test]
    fn a_stored_mesh_is_written_on_a_handful_of_lines() {
        // The reason for the encoding: a converted tile must not turn the
        // project file into something no editor will open.
        let mesh = primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 64);
        assert!(mesh.triangle_count() > 4000, "the test needs a big mesh");
        let text = serde_json::to_string_pretty(&MeshData::new(mesh).to_blob()).unwrap();
        assert!(text.lines().count() < 10, "{} lines", text.lines().count());
    }
}

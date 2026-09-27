//! STL, binary and text.
//!
//! Binary versus text is decided by size arithmetic (84 bytes plus 50 per triangle), since many
//! binary headers start with `solid`. Facet normals are ignored in favour of the winding, since
//! other programs often get them inconsistent.

use super::*;
use simple3d_geom::Vec3;

/// Whether these bytes are an STL at all, for [`Format::sniff`].
pub(crate) fn looks_like_stl(bytes: &[u8]) -> bool {
    binary_count(bytes).is_some() || looks_like_text(bytes)
}

/// A text STL begins with `solid` and has at least one facet; the word alone also starts many
/// binary headers.
fn looks_like_text(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(4096)];
    let head = text(head).to_lowercase();
    head.trim_start().starts_with("solid") && head.contains("facet")
}

/// The triangle count a binary STL's size proves, or `None` if it does not add up.
fn binary_count(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 84 {
        return None;
    }
    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    // Trailing padding is tolerated, but a file shorter than its count is not an STL.
    (count > 0 && bytes.len() >= 84 + count * 50).then_some(count)
}

pub(crate) fn read(bytes: &[u8], progress: Progress<'_>) -> Result<Model, ImportError> {
    // Text if it reads as text throughout, binary if the size adds up, else text as a last resort.
    if looks_like_text(bytes) && exactly_text(bytes) {
        return ascii(bytes, progress);
    }
    match binary_count(bytes) {
        Some(count) => binary(bytes, count, progress),
        None if looks_like_text(bytes) => ascii(bytes, progress),
        None => Err(malformed("the file is neither a whole number of binary STL triangles nor text facets")),
    }
}

/// Whether text-looking bytes are really text: all printable or whitespace, which binary floats are not.
fn exactly_text(bytes: &[u8]) -> bool {
    bytes.iter().take(4096).all(|&b| b == b'\t' || b == b'\n' || b == b'\r' || (0x20..0x7f).contains(&b))
}

fn binary(bytes: &[u8], count: usize, mut progress: Progress<'_>) -> Result<Model, ImportError> {
    let mut mesh = Mesh::new();
    mesh.positions.reserve(count * 3);
    mesh.indices.reserve(count);
    for triangle in 0..count {
        let at = 84 + triangle * 50;
        // Skip the normal at `at`; the next bytes are the vertices in winding order.
        let vertex = |i: usize| -> Vec3 {
            let at = at + 12 + i * 12;
            let f = |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as f64;
            Vec3::new(f(at), f(at + 4), f(at + 8))
        };
        let (a, b, c) = (vertex(0), vertex(1), vertex(2));
        if ![a, b, c].iter().all(|v| v.x.is_finite() && v.y.is_finite() && v.z.is_finite()) {
            return Err(malformed(format!("triangle {} has a coordinate that is not a number", triangle + 1)));
        }
        mesh.push_triangle(a, b, c);
        if triangle % 8192 == 0 {
            step(&mut progress, 0.05 + 0.9 * (triangle as f32 / count as f32))?;
        }
    }
    Ok(Model { format: Format::Stl, unit: None, parts: vec![Part { name: String::new(), mesh }] })
}

/// One part per `solid`, since some programs write an assembly as several solids.
fn ascii(bytes: &[u8], mut progress: Progress<'_>) -> Result<Model, ImportError> {
    let text = text(bytes);
    let lines = text.lines().count().max(1);
    let mut parts: Vec<Part> = Vec::new();
    let mut open: Option<Part> = None;
    let mut loop_vertices: Vec<Vec3> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let mut words = line.split_whitespace();
        let Some(keyword) = words.next() else { continue };
        match keyword.to_lowercase().as_str() {
            "solid" => {
                if let Some(part) = open.take() {
                    parts.push(part);
                }
                open = Some(Part { name: words.collect::<Vec<_>>().join(" "), mesh: Mesh::new() });
            }
            "endsolid" => {
                if let Some(part) = open.take() {
                    parts.push(part);
                }
            }
            "vertex" => {
                let coordinates: Vec<&str> = words.collect();
                if coordinates.len() < 3 {
                    return Err(malformed(format!("line {}: a vertex needs three coordinates", number + 1)));
                }
                let mut read = [0.0f64; 3];
                for (slot, word) in read.iter_mut().zip(&coordinates) {
                    *slot = word
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| malformed(format!("line {}: {word:?} is not a coordinate", number + 1)))?;
                }
                loop_vertices.push(Vec3::new(read[0], read[1], read[2]));
            }
            "endloop" => {
                // A loop of more than three vertices is fanned, keeping the whole polygon.
                if loop_vertices.len() >= 3 {
                    let part = open.get_or_insert_with(|| Part { name: String::new(), mesh: Mesh::new() });
                    for i in 1..loop_vertices.len() - 1 {
                        part.mesh.push_triangle(loop_vertices[0], loop_vertices[i], loop_vertices[i + 1]);
                    }
                } else if !loop_vertices.is_empty() {
                    return Err(malformed(format!("line {}: a facet with fewer than three vertices", number + 1)));
                }
                loop_vertices.clear();
            }
            _ => {}
        }
        if number % 8192 == 0 {
            step(&mut progress, 0.05 + 0.9 * (number as f32 / lines as f32))?;
        }
    }
    if let Some(part) = open.take() {
        parts.push(part);
    }
    // Empty solids are dropped; a file of only those is reported empty by the caller.
    parts.retain(|part| part.mesh.triangle_count() > 0);
    Ok(Model { format: Format::Stl, unit: None, parts })
}

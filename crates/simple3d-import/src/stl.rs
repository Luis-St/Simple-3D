//! STL, binary and text.
//!
//! Which of the two a file is is decided by arithmetic, not by its first word:
//! a binary STL is exactly 84 bytes of header and triangle count plus 50 bytes
//! per triangle, and plenty of programs write the word `solid` into that
//! 80-byte header -- so trusting the word is how a binary file gets read as
//! text and comes back empty.
//!
//! The facet normals are read past. A normal is derivable from the winding and
//! the two disagree often enough in files from other programs that only one of
//! them can be believed; the winding is the one the rest of this workspace
//! works in, and [`simple3d_geom::repair`] is what a caller applies when a
//! surface turns out to be inconsistent.

use super::*;
use simple3d_geom::Vec3;

/// Whether these bytes are an STL at all, for [`Format::sniff`].
pub(crate) fn looks_like_stl(bytes: &[u8]) -> bool {
    binary_count(bytes).is_some() || looks_like_text(bytes)
}

/// A text STL begins with `solid` and has at least one facet in it. Both
/// halves matter: the word alone is also how a binary file's header often
/// starts, and a file of nothing but `solid x` holds no geometry to read.
fn looks_like_text(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(4096)];
    let head = text(head).to_lowercase();
    head.trim_start().starts_with("solid") && head.contains("facet")
}

/// The triangle count a binary STL's size proves it holds, `None` when the
/// size does not add up to a whole number of triangles.
fn binary_count(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 84 {
        return None;
    }
    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    // Trailing bytes are tolerated -- some writers pad the end -- but a file
    // shorter than its own count is not an STL.
    (count > 0 && bytes.len() >= 84 + count * 50).then_some(count)
}

pub(crate) fn read(bytes: &[u8], progress: Progress<'_>) -> Result<Model, ImportError> {
    // Text when the file reads as text all the way through, binary when the
    // size adds up, and text as a last resort for a file that is mostly text
    // with something odd in it -- a comment in another code page, say.
    if looks_like_text(bytes) && exactly_text(bytes) {
        return ascii(bytes, progress);
    }
    match binary_count(bytes) {
        Some(count) => binary(bytes, count, progress),
        None if looks_like_text(bytes) => ascii(bytes, progress),
        None => Err(malformed("the file is neither a whole number of binary STL triangles nor text facets")),
    }
}

/// Whether a file that looks like text really is text: every byte printable or
/// whitespace. A binary STL's header can spell `solid` and its triangles are
/// floating-point bytes, which this refuses.
fn exactly_text(bytes: &[u8]) -> bool {
    bytes.iter().take(4096).all(|&b| b == b'\t' || b == b'\n' || b == b'\r' || (0x20..0x7f).contains(&b))
}

fn binary(bytes: &[u8], count: usize, mut progress: Progress<'_>) -> Result<Model, ImportError> {
    let mut mesh = Mesh::new();
    mesh.positions.reserve(count * 3);
    mesh.indices.reserve(count);
    for triangle in 0..count {
        let at = 84 + triangle * 50;
        // The normal at `at` is read past; the twelve bytes after it are the
        // vertices, in winding order.
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

/// One part per `solid`, because that is the only structure a text STL has and
/// some programs write an assembly as several solids in one file.
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
                // A facet is three vertices; a loop with more is triangulated
                // as a fan, which is the only reading of a flat polygon that
                // does not throw part of it away.
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
    // Solids that hold nothing are dropped rather than becoming empty rows in
    // the outliner; a file of nothing but those is reported as empty by the
    // caller.
    parts.retain(|part| part.mesh.triangle_count() > 0);
    Ok(Model { format: Format::Stl, unit: None, parts })
}

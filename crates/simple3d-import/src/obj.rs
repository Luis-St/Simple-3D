//! OBJ.
//!
//! Vertices are numbered file-wide, so each part collects its faces first and then remaps only the
//! vertices it uses. Coordinates are taken as written (OBJ has no unit or up axis); the node can be
//! rotated afterwards.

use super::*;
use simple3d_geom::Vec3;

/// A face as the file names it: indices into the file's vertex list.
type Face = Vec<usize>;

pub(crate) fn read(bytes: &[u8], mut progress: Progress<'_>) -> Result<Model, ImportError> {
    let text = text(bytes);
    let lines = text.lines().count().max(1);
    let mut positions: Vec<Vec3> = Vec::new();
    // Every group so far with its faces; the unnamed first one holds everything before any `o` or `g`.
    let mut groups: Vec<(String, Vec<Face>)> = vec![(String::new(), Vec::new())];
    for (number, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("");
        let mut words = line.split_whitespace();
        let Some(keyword) = words.next() else { continue };
        match keyword {
            "v" => {
                let mut read = [0.0f64; 3];
                for slot in read.iter_mut() {
                    let word = words
                        .next()
                        .ok_or_else(|| malformed(format!("line {}: a vertex needs three coordinates", number + 1)))?;
                    *slot = word
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| malformed(format!("line {}: {word:?} is not a coordinate", number + 1)))?;
                }
                positions.push(Vec3::new(read[0], read[1], read[2]));
            }
            // `o` and `g` both start a part, since programs disagree which one an object is.
            "o" | "g" => {
                let name = words.collect::<Vec<_>>().join(" ");
                // A group named before any face replaces the empty previous one, so `o` then `g` leaves no empty part.
                match groups.last_mut() {
                    Some((current, faces)) if faces.is_empty() => *current = name,
                    _ => groups.push((name, Vec::new())),
                }
            }
            "f" => {
                let mut face: Face = Vec::new();
                for word in words {
                    // `v`, `v/vt`, `v/vt/vn`, `v//vn`: only the vertex index is geometry.
                    let field = word.split('/').next().unwrap_or("");
                    let index = field
                        .parse::<isize>()
                        .ok()
                        .ok_or_else(|| malformed(format!("line {}: {word:?} is not a vertex index", number + 1)))?;
                    // Negative indices count back from the vertices read so far.
                    let resolved = if index < 0 {
                        positions.len().checked_sub(index.unsigned_abs())
                    } else {
                        (index as usize).checked_sub(1)
                    };
                    let resolved = resolved.filter(|&i| i < positions.len()).ok_or_else(|| {
                        malformed(format!("line {}: vertex {index} is not one the file has defined", number + 1))
                    })?;
                    face.push(resolved);
                }
                if face.len() >= 3 {
                    groups.last_mut().expect("there is always a group").1.push(face);
                }
            }
            _ => {}
        }
        if number % 8192 == 0 {
            step(&mut progress, 0.05 + 0.8 * (number as f32 / lines as f32))?;
        }
    }

    let parts = groups
        .into_iter()
        .filter(|(_, faces)| !faces.is_empty())
        .map(|(name, faces)| Part { name, mesh: build(&positions, &faces) })
        .collect();
    step(&mut progress, 0.95)?;
    Ok(Model { format: Format::Obj, unit: None, parts })
}

/// One part's mesh: its used vertices in first-use order, and faces fanned into triangles.
fn build(positions: &[Vec3], faces: &[Face]) -> Mesh {
    let mut mesh = Mesh::new();
    let mut mapped: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    for face in faces {
        let mut indices = Vec::with_capacity(face.len());
        for &vertex in face {
            let index = *mapped.entry(vertex).or_insert_with(|| {
                mesh.positions.push(positions[vertex]);
                (mesh.positions.len() - 1) as u32
            });
            indices.push(index);
        }
        for i in 1..indices.len() - 1 {
            // Skip degenerate corners naming a vertex twice; they would have no area.
            if indices[0] != indices[i] && indices[i] != indices[i + 1] && indices[0] != indices[i + 1] {
                mesh.indices.push([indices[0], indices[i], indices[i + 1]]);
                mesh.tags.push(0);
            }
        }
    }
    mesh
}

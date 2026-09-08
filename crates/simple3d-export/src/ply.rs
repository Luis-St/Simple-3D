//! PLY, ASCII and binary.

use super::*;
use simple3d_geom::Mesh;

pub(crate) fn ply_header(mesh: &Mesh, binary: bool) -> String {
    let format = if binary { "binary_little_endian 1.0" } else { "ascii 1.0" };
    format!(
        "ply\nformat {format}\ncomment Exported by Simple 3D\ncomment Units: millimetres\n\
         element vertex {}\nproperty double x\nproperty double y\nproperty double z\n\
         element face {}\nproperty list uchar uint vertex_indices\nend_header\n",
        mesh.positions.len(),
        mesh.indices.len()
    )
}

pub(crate) fn ply_ascii(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = ply_header(mesh, false);
    for (i, p) in mesh.positions.iter().enumerate() {
        out.push_str(&format!("{} {} {}\n", coord(p.x), coord(p.y), coord(p.z)));
        if i % 8192 == 0 && !progress(0.2 + 0.4 * (i as f32 / mesh.positions.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    for (i, t) in mesh.indices.iter().enumerate() {
        out.push_str(&format!("3 {} {} {}\n", t[0], t[1], t[2]));
        if i % 8192 == 0 && !progress(0.6 + 0.3 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    Ok(out.into_bytes())
}

pub(crate) fn ply_binary(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = ply_header(mesh, true).into_bytes();
    for (i, p) in mesh.positions.iter().enumerate() {
        out.extend_from_slice(&p.x.to_le_bytes());
        out.extend_from_slice(&p.y.to_le_bytes());
        out.extend_from_slice(&p.z.to_le_bytes());
        if i % 8192 == 0 && !progress(0.2 + 0.4 * (i as f32 / mesh.positions.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    for (i, t) in mesh.indices.iter().enumerate() {
        out.push(3);
        for &vertex in t {
            out.extend_from_slice(&vertex.to_le_bytes());
        }
        if i % 8192 == 0 && !progress(0.6 + 0.3 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    Ok(out)
}

//! OBJ.

use super::*;
use simple3d_geom::Mesh;

pub(crate) fn obj(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = String::with_capacity(mesh.positions.len() * 32 + mesh.indices.len() * 24);
    out.push_str("# Exported by Simple 3D\n# Units: millimetres\n");
    out.push_str("o simple3d\n");
    for (i, p) in mesh.positions.iter().enumerate() {
        out.push_str(&format!("v {} {} {}\n", coord(p.x), coord(p.y), coord(p.z)));
        if i % 8192 == 0 && !progress(0.2 + 0.4 * (i as f32 / mesh.positions.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    for (i, t) in mesh.indices.iter().enumerate() {
        // OBJ indices are 1-based.
        out.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
        if i % 8192 == 0 && !progress(0.6 + 0.3 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    Ok(out.into_bytes())
}

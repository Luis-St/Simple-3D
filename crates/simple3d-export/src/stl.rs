//! STL, binary and ASCII.

use super::*;
use simple3d_geom::{Mesh, Vec3};

pub(crate) fn stl_binary(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = Vec::with_capacity(84 + mesh.indices.len() * 50);
    let mut header = [0u8; 80];
    let banner = b"Exported by Simple 3D";
    header[..banner.len()].copy_from_slice(banner);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(mesh.indices.len() as u32).to_le_bytes());
    for (i, tri) in mesh.indices.iter().enumerate() {
        let n = mesh.triangle_normal(*tri);
        push_f32(&mut out, n);
        for &vertex in tri {
            push_f32(&mut out, mesh.positions[vertex as usize]);
        }
        out.extend_from_slice(&0u16.to_le_bytes()); // attribute byte count
        if i % 8192 == 0 && !progress(0.2 + 0.7 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    Ok(out)
}

pub(crate) fn push_f32(out: &mut Vec<u8>, v: Vec3) {
    out.extend_from_slice(&(v.x as f32).to_le_bytes());
    out.extend_from_slice(&(v.y as f32).to_le_bytes());
    out.extend_from_slice(&(v.z as f32).to_le_bytes());
}

pub(crate) fn stl_ascii(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = String::with_capacity(mesh.indices.len() * 180);
    out.push_str("solid simple3d\n");
    for (i, tri) in mesh.indices.iter().enumerate() {
        let n = mesh.triangle_normal(*tri);
        out.push_str(&format!("  facet normal {} {} {}\n", coord(n.x), coord(n.y), coord(n.z)));
        out.push_str("    outer loop\n");
        for &vertex in tri {
            let p = mesh.positions[vertex as usize];
            out.push_str(&format!("      vertex {} {} {}\n", coord(p.x), coord(p.y), coord(p.z)));
        }
        out.push_str("    endloop\n  endfacet\n");
        if i % 4096 == 0 && !progress(0.2 + 0.7 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    out.push_str("endsolid simple3d\n");
    Ok(out.into_bytes())
}

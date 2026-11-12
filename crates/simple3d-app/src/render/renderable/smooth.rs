//! Smooth shading for mesh bodies: which triangles get it, their corner normals, and the edges
//! still drawn across them.

use super::*;
use simple3d_geom::topology::{corner_normals, vertex_faces};
use std::collections::BTreeSet;

/// How sharply flat-shaded faces must meet to draw a line between them.
pub(crate) const EDGE_CREASE: f64 = 20.0;

/// How sharply a mesh body's faces must meet to shade as a corner, and to draw a line there; kept
/// with the geometry, whose refining curves the surface by the same rule.
pub(crate) use simple3d_geom::topology::SMOOTH_CREASE;

/// Whether each triangle of `mesh` shades smooth: those whose source is in `smooth`
/// (`Evaluated::smooth`). Empty when none does, so flat items carry nothing extra.
pub(super) fn smooth_flags(mesh: &Mesh, smooth: &BTreeSet<u32>) -> Vec<bool> {
    if smooth.is_empty() {
        return Vec::new();
    }
    let flags: Vec<bool> = (0..mesh.indices.len()).map(|index| smooth.contains(&mesh.source(index))).collect();
    if flags.contains(&true) {
        flags
    } else {
        Vec::new()
    }
}

/// The smooth triangles' corner normals ([`Renderable::corner_normals`]); empty without `flags`.
pub(super) fn corners(mesh: &Mesh, table: &EdgeTable, flags: &[bool]) -> Vec<[Vec3; 3]> {
    if flags.is_empty() {
        return Vec::new();
    }
    corner_normals(mesh, table, &vertex_faces(mesh), SMOOTH_CREASE, flags)
}

/// The feature edges and the triangles beside each: [`SMOOTH_CREASE`] between two smooth triangles,
/// [`EDGE_CREASE`] elsewhere, so a mesh body's facets are not lined but where it meets a cut still is.
pub(super) fn feature_edges(table: &EdgeTable, normals: &[Vec3], flags: &[bool]) -> (Vec<[u32; 2]>, Vec<[u32; 2]>) {
    if flags.is_empty() {
        return table.feature_edges(normals, EDGE_CREASE);
    }
    let (flat, smooth) = (EDGE_CREASE.to_radians().cos(), SMOOTH_CREASE.to_radians().cos());
    table.feature_edges_by(normals, |a, b| if flags[a as usize] && flags[b as usize] { smooth } else { flat })
}

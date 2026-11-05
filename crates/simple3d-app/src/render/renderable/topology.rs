//! The renderer's view of a welded mesh's connectivity, read from [`simple3d_geom::topology`].

use super::*;
pub(super) use simple3d_geom::topology::EdgeTable;

/// Each vertex's connected body, as the card stores it. Past `u16::MAX` bodies they share the
/// last number.
pub(crate) fn bodies_of(mesh: &Mesh) -> Vec<u16> {
    let cap = u32::from(u16::MAX) - 1;
    simple3d_geom::topology::connected_bodies(mesh).into_iter().map(|body| body.min(cap) as u16).collect()
}

/// Real creases plus single-triangle edges; every triangle edge would just be noise.
/// Test entry point.
#[cfg(test)]
pub fn feature_edges(mesh: &Mesh, angle_deg: f64) -> Vec<[u32; 2]> {
    let normals: Vec<Vec3> = mesh.indices.iter().map(|tri| mesh.triangle_normal(*tri)).collect();
    EdgeTable::of(mesh).feature_edges(&normals, angle_deg).0
}

/// Every edge with the triangles that meet at it, in a deterministic order.
#[cfg(test)]
pub(crate) fn border_edges(mesh: &Mesh) -> Vec<BorderEdge> {
    border_edges_of(&EdgeTable::of(mesh))
}

/// Every edge of `table` for the selection outline. Anything but two faces is always drawn;
/// `push_selection` reads a repeated triangle as "no far side".
pub(super) fn border_edges_of(table: &EdgeTable) -> Vec<BorderEdge> {
    table
        .iter()
        .map(|(ends, faces)| {
            let pair = match faces {
                [a, b] => [*a, *b],
                _ => [faces[0]; 2],
            };
            BorderEdge { ends, faces: pair, junction: faces.len() > 2 }
        })
        .collect()
}

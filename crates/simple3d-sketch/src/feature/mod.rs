//! Solids made from a sketch's picked regions: extruded along the sketch's normal, or revolved about
//! a line of the sketch. Both work in the sketch's own frame, the plane at z = 0.

mod extrude;
pub use extrude::{extrude, Extrude};
mod offset;
mod revolve;
pub use revolve::revolve;

use simple3d_geom::push_pull::Outline;
use simple3d_geom::Vec3;

/// The caps of `outlines` lifted to height `z`, as triangles of positions, counter-clockwise seen
/// from +Z. `None` if a cap cannot be laid over an outline.
pub(crate) fn caps(outlines: &[Outline], z: f64) -> Option<Vec<[Vec3; 3]>> {
    let mut out = Vec::new();
    for outline in outlines {
        let mut positions: Vec<Vec3> = Vec::new();
        let mut ids: Vec<Vec<u32>> = Vec::new();
        for points in std::iter::once(&outline.outer).chain(outline.holes.iter()) {
            let start = positions.len() as u32;
            ids.push((0..points.len() as u32).map(|k| start + k).collect());
            positions.extend(points.iter().map(|p| Vec3::new(p[0], p[1], z)));
        }
        let triangles = simple3d_geom::planar::triangulate_loops(&positions, Vec3::new(0.0, 0.0, 1.0), ids)?;
        out.extend(triangles.iter().map(|t| t.map(|i| positions[i as usize])));
    }
    Some(out)
}

/// Every loop of every outline, outer edges counter-clockwise and holes clockwise, so the solid is
/// always on the left of travel.
pub(crate) fn loops(outlines: &[Outline]) -> Vec<&Vec<[f64; 2]>> {
    outlines.iter().flat_map(|o| std::iter::once(&o.outer).chain(o.holes.iter())).collect()
}

/// Whether a triangle has no area worth keeping.
pub(crate) fn degenerate(a: Vec3, b: Vec3, c: Vec3) -> bool {
    (b - a).cross(c - a).length() < 1e-12
}

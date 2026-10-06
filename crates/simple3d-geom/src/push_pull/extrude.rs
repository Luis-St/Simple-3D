//! An outline swept straight up into a closed solid.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// The outline extruded from z = 0 to z = `distance`, walls and both caps, wound outward. Empty for a
/// distance too small to be a solid or an outline the caps cannot be laid over.
pub fn extrude_outline(outline: &Outline, distance: f64) -> Mesh {
    let mut mesh = Mesh::new();
    if distance.is_nan() || distance <= 1e-6 || outline.outer.len() < 3 {
        return mesh;
    }
    let loops: Vec<&Vec<[f64; 2]>> = std::iter::once(&outline.outer).chain(outline.holes.iter()).collect();
    // The top cap's corners, numbered loop after loop, so the caps can be triangulated by index.
    let mut top: Vec<Vec3> = Vec::new();
    let mut ids: Vec<Vec<u32>> = Vec::new();
    for points in &loops {
        ids.push(points.iter().map(|_| top.len() as u32).zip(0..).map(|(start, k)| start + k).collect());
        top.extend(points.iter().map(|p| Vec3::new(p[0], p[1], distance)));
    }
    let Some(cap) = crate::planar::triangulate_loops(&top, Vec3::new(0.0, 0.0, 1.0), ids) else {
        return mesh;
    };
    let lower = |p: Vec3| Vec3::new(p.x, p.y, 0.0);
    for tri in &cap {
        let [a, b, c] = tri.map(|i| top[i as usize]);
        mesh.push_triangle(a, b, c);
        mesh.push_triangle(lower(a), lower(c), lower(b));
    }
    // Outer loop counter-clockwise and holes clockwise both put the solid on the left of travel, so
    // one winding faces every wall outward.
    for points in &loops {
        let n = points.len();
        for i in 0..n {
            let (p, q) = (points[i], points[(i + 1) % n]);
            let (b0, b1) = (Vec3::new(p[0], p[1], 0.0), Vec3::new(q[0], q[1], 0.0));
            let (t0, t1) = (Vec3::new(p[0], p[1], distance), Vec3::new(q[0], q[1], distance));
            mesh.push_triangle(b0, b1, t1);
            mesh.push_triangle(b0, t1, t0);
        }
    }
    mesh
}

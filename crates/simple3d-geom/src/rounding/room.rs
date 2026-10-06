//! How large a treatment an edge has room for: a round or bevel reaching further along a face than
//! the face goes would cut through whatever lies beyond it, so such an edge is left untreated.

use super::*;
use crate::mesh::Mesh;
use crate::push_pull::flat_face;
use crate::vec3::Vec3;

/// How far back from `edge` the treatment may reach along both its faces in `mesh`: the nearest the
/// boundary of either face comes, seen from points along the edge. Infinite where a face is not found.
pub fn edge_room(mesh: &Mesh, edge: &FeatureEdge) -> f64 {
    (0..2).map(|side| face_room(mesh, edge, side)).fold(f64::INFINITY, f64::min)
}

/// Whether `edge`, with `room` from [`edge_room`], can take `treatment`. Reaching exactly to the far
/// side of a face is allowed: a plate as thick as the radius rounds into a half-round.
pub fn edge_fits(edge: &FeatureEdge, treatment: Treatment, room: f64) -> bool {
    treatment.reach(edge.opening()) <= room + 1e-6
}

/// Whether `corner` can take `treatment` without reaching past the far end of one of its edges.
pub fn corner_fits(corner: &Corner, treatment: Treatment) -> bool {
    let size = match treatment {
        Treatment::Round { radius, .. } => radius,
        Treatment::Chamfer { distance } => distance,
    };
    corner.edges.iter().all(|&(_, length)| size < length)
}

fn face_room(mesh: &Mesh, edge: &FeatureEdge, side: usize) -> f64 {
    let (normal, w) = (edge.normals[side], edge.along[side]);
    let (t, length) = (edge.direction(), edge.length());
    let probe = (edge.a + edge.b) * 0.5 + w * (length * 1e-3).min(1e-3);
    let Some(triangle) = (0..mesh.indices.len()).find(|&i| holds(mesh, i, normal, probe)) else {
        return f64::INFINITY;
    };
    let Some(face) = flat_face(mesh, triangle) else { return f64::INFINITY };
    // In the face's plane: x along the edge, y away from it into the face.
    let flat = |p: Vec3| ((p - edge.a).dot(t), (p - edge.a).dot(w));
    let mut room = f64::INFINITY;
    for s in [0.1, 0.3, 0.5, 0.7, 0.9] {
        let x = length * s;
        for points in &face.loops {
            for (i, &p) in points.iter().enumerate() {
                let ((px, py), (qx, qy)) = (flat(p), flat(points[(i + 1) % points.len()]));
                if (px - x) * (qx - x) > 0.0 || (qx - px).abs() < 1e-12 {
                    continue;
                }
                let y = py + (x - px) / (qx - px) * (qy - py);
                // The edge itself bounds the face at y = 0.
                if y > 1e-6 {
                    room = room.min(y);
                }
            }
        }
    }
    room
}

/// Whether triangle `i` faces along `normal` and holds `p`.
fn holds(mesh: &Mesh, i: usize, normal: Vec3, p: Vec3) -> bool {
    let tri = mesh.indices[i];
    let n = mesh.triangle_normal(tri);
    let [a, b, c] = mesh.corners(tri);
    if n.dot(normal) < 1.0 - 1e-6 || (p - a).dot(n).abs() > 1e-4 {
        return false;
    }
    let inside = |u: Vec3, v: Vec3| (v - u).cross(p - u).dot(n) >= -1e-9;
    inside(a, b) && inside(b, c) && inside(c, a)
}

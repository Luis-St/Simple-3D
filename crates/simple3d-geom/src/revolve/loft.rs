//! Joining closed rings of points into a solid, for shapes no single outline or profile describes.

use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Join closed rings of equal length, ring k to ring k+1 point by point. A `closed` sequence also joins
/// the last ring back to the first, as a hollow shape's inner wall returns to its outer one; otherwise
/// the first and last rings are capped, fanned from their centroid, so they must be star-shaped about
/// it. A ring may repeat one point to close at an apex: the triangles that collapse are dropped when
/// the mesh is welded. Wound outward whichever way the rings run.
pub fn grid_mesh(rings: &[Vec<Vec3>], closed: bool) -> Mesh {
    let mut mesh = Mesh::new();
    let count = rings.len();
    if count < 2 {
        return mesh;
    }
    let n = rings[0].len();
    let joins = if closed { count } else { count - 1 };
    for k in 0..joins {
        let (a, b) = (&rings[k], &rings[(k + 1) % count]);
        debug_assert_eq!(a.len(), b.len());
        for i in 0..n {
            let j = (i + 1) % n;
            mesh.push_triangle(a[i], a[j], b[j]);
            mesh.push_triangle(a[i], b[j], b[i]);
        }
    }
    if !closed {
        let centroid = |ring: &[Vec3]| ring.iter().fold(Vec3::ZERO, |sum, &p| sum + p) * (1.0 / n as f64);
        let (first, last) = (&rings[0], &rings[count - 1]);
        let (c0, c1) = (centroid(first), centroid(last));
        for i in 0..n {
            let j = (i + 1) % n;
            mesh.push_triangle(c0, first[j], first[i]);
            mesh.push_triangle(c1, last[i], last[j]);
        }
    }
    mesh.turned_outward()
}

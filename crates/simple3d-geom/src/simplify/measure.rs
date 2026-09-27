//! How far the simplified surface actually ended up from the original.
//!
//! The quadric is a mean estimate and understates the worst spot, so the result is measured
//! afterwards: every original vertex against the new surface, the direction that catches a bump
//! collapsed flat. A uniform grid over the result's triangles keeps each query to a few cells.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// The furthest any point of `from` is from the surface of `to`, in millimetres.
pub fn furthest_from(from: &[Vec3], to: &Mesh) -> f64 {
    let Some(grid) = Grid::of(to) else { return 0.0 };
    from.iter().map(|&p| grid.distance(to, p)).fold(0.0, f64::max)
}

/// The furthest the two surfaces are from each other, in both directions, in millimetres. One
/// direction alone calls a drilled plate a box.
pub fn furthest_between(a: &Mesh, b: &Mesh) -> f64 {
    furthest_from(&a.positions, b).max(furthest_from(&b.positions, a))
}

/// The triangles of a mesh, bucketed by position.
struct Grid {
    cell: f64,
    lo: Vec3,
    /// The occupied cell range, so a query from outside knows when it has passed the far side.
    extent: ((i32, i32, i32), (i32, i32, i32)),
    buckets: HashMap<(i32, i32, i32), Vec<u32>>,
    /// Triangles too big to bucket, which would otherwise fill the grid several times over.
    everywhere: Vec<u32>,
}

/// How many cells a triangle may span before it counts as everywhere.
const SPAN: i32 = 3;

impl Grid {
    fn of(mesh: &Mesh) -> Option<Grid> {
        let (lo, hi) = mesh.bounds()?;
        if mesh.indices.is_empty() {
            return None;
        }
        // A cell about the size of a triangle: the longest box side over the cube root of the triangle count.
        let span = (hi - lo).length().max(1e-9);
        let cell = (span / (mesh.indices.len() as f64).cbrt()).max(1e-6);
        let mut grid =
            Grid { cell, lo, extent: ((0, 0, 0), (0, 0, 0)), buckets: HashMap::new(), everywhere: Vec::new() };
        grid.extent = (grid.cell_of(lo), grid.cell_of(hi));
        for (index, tri) in mesh.indices.iter().enumerate() {
            let points = mesh.corners(*tri);
            let (tlo, thi) = points.iter().fold((points[0], points[0]), |(lo, hi), &p| (lo.min(p), hi.max(p)));
            let (a, b) = (grid.cell_of(tlo), grid.cell_of(thi));
            if (b.0 - a.0).max(b.1 - a.1).max(b.2 - a.2) > SPAN {
                grid.everywhere.push(index as u32);
                continue;
            }
            for x in a.0..=b.0 {
                for y in a.1..=b.1 {
                    for z in a.2..=b.2 {
                        grid.buckets.entry((x, y, z)).or_default().push(index as u32);
                    }
                }
            }
        }
        Some(grid)
    }

    fn cell_of(&self, p: Vec3) -> (i32, i32, i32) {
        let at = |v: f64, lo: f64| ((v - lo) / self.cell).floor() as i32;
        (at(p.x, self.lo.x), at(p.y, self.lo.y), at(p.z, self.lo.z))
    }

    /// The distance from `p` to the nearest triangle, searching rings of cells outwards until the
    /// next ring cannot beat the best found.
    fn distance(&self, mesh: &Mesh, p: Vec3) -> f64 {
        let mut best = f64::MAX;
        for &index in &self.everywhere {
            best = best.min(point_to_triangle(p, mesh.corners(mesh.indices[index as usize])));
        }
        let at = self.cell_of(p);
        let (lo, hi) = self.extent;
        // Past this reach the rings are entirely outside the mesh's box.
        let reach = [(at.0, lo.0, hi.0), (at.1, lo.1, hi.1), (at.2, lo.2, hi.2)]
            .iter()
            .map(|&(c, lo, hi)| (c - lo).abs().max((c - hi).abs()))
            .max()
            .unwrap_or(0);
        for ring in 0..=reach {
            // Nothing beyond this ring can be closer than its walls.
            if best <= f64::from(ring) * self.cell {
                return best;
            }
            for x in (at.0 - ring)..=(at.0 + ring) {
                for y in (at.1 - ring)..=(at.1 + ring) {
                    for z in (at.2 - ring)..=(at.2 + ring) {
                        // Only the ring's shell; the inside was walked by the previous ring.
                        let shell = (x - at.0).abs() == ring || (y - at.1).abs() == ring || (z - at.2).abs() == ring;
                        if !shell {
                            continue;
                        }
                        let Some(bucket) = self.buckets.get(&(x, y, z)) else { continue };
                        for &index in bucket {
                            best = best.min(point_to_triangle(p, mesh.corners(mesh.indices[index as usize])));
                        }
                    }
                }
            }
        }
        best
    }
}

/// The distance from a point to a triangle: to the face if the projection lands inside, else to
/// the nearest edge.
pub(super) fn point_to_triangle(p: Vec3, tri: [Vec3; 3]) -> f64 {
    let [a, b, c] = tri;
    let normal = (b - a).cross(c - a);
    let area = normal.length();
    if area > 0.0 {
        let n = normal * (1.0 / area);
        let on_plane = p - n * (p - a).dot(n);
        // Inside when on the same side of all three edges.
        let inside =
            [(a, b), (b, c), (c, a)].iter().all(|&(from, to)| (to - from).cross(on_plane - from).dot(n) >= 0.0);
        if inside {
            return (p - on_plane).length();
        }
    }
    [(a, b), (b, c), (c, a)].iter().map(|&(from, to)| point_to_segment(p, from, to)).fold(f64::MAX, f64::min)
}

fn point_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let along = b - a;
    let length = along.dot(along);
    let t = if length > 0.0 { ((p - a).dot(along) / length).clamp(0.0, 1.0) } else { 0.0 };
    (p - (a + along * t)).length()
}

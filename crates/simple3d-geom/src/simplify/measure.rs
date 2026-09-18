//! How far the simplified surface actually ended up from the original.
//!
//! The quadric the collapses are ordered by is an *estimate* of that, and a
//! good one for choosing what to drop next, but it is not the number to put in
//! front of somebody: it is an area-weighted mean of squared distances to the
//! planes a vertex stands for, which is smaller than the worst place on the
//! surface by however unevenly the error is spread. A number that says the
//! surface moved 0.16 mm where it has really moved 1 mm is worse than no number
//! at all.
//!
//! So the run finishes and then measures, here: every vertex of the original
//! against the surface that replaced it. That is the direction that catches
//! what matters -- a bump collapsed flat leaves every *new* vertex sitting
//! exactly on the old surface while the tip of the bump is a millimetre from
//! the new one -- and vertices are where a collapse leaves its extremes, since
//! the surface between them is what was interpolated away.
//!
//! Brute force over the triangles would be one distance per pair, which for a
//! mesh of any size is the slowest thing in the tool by two orders of
//! magnitude. A uniform grid over the result's triangles turns it into a
//! handful of cells per point, and the search stops as soon as the ring it is
//! looking at cannot hold anything closer than what it already has.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// The furthest any point of `from` ends up from the surface of `to`, in
/// millimetres.
pub(crate) fn furthest_from(from: &[Vec3], to: &Mesh) -> f64 {
    let Some(grid) = Grid::of(to) else { return 0.0 };
    from.iter().map(|&p| grid.distance(to, p)).fold(0.0, f64::max)
}

/// The triangles of a mesh, in buckets by where they are.
struct Grid {
    cell: f64,
    lo: Vec3,
    /// The cells the mesh actually occupies, so a query that starts outside it
    /// knows when it has walked past the far side rather than expanding for
    /// ever.
    extent: ((i32, i32, i32), (i32, i32, i32)),
    buckets: HashMap<(i32, i32, i32), Vec<u32>>,
    /// Triangles too big to bucket: a shape simplified to a few dozen faces has
    /// triangles that span it, and putting one in every cell it touches would
    /// be the whole grid several times over.
    everywhere: Vec<u32>,
}

/// How many cells across a triangle may reach before it is put aside as one
/// that is everywhere. Three is a triangle a little bigger than a cell, which
/// the sizing below makes the ordinary case.
const SPAN: i32 = 3;

impl Grid {
    fn of(mesh: &Mesh) -> Option<Grid> {
        let (lo, hi) = mesh.bounds()?;
        if mesh.indices.is_empty() {
            return None;
        }
        // A cell about the size of a triangle: the box's longest side over the
        // cube root of the triangle count. Smaller means empty cells to walk
        // through, larger means every query reading half the mesh.
        let span = (hi - lo).length().max(1e-9);
        let cell = (span / (mesh.indices.len() as f64).cbrt()).max(1e-6);
        let mut grid =
            Grid { cell, lo, extent: ((0, 0, 0), (0, 0, 0)), buckets: HashMap::new(), everywhere: Vec::new() };
        grid.extent = (grid.cell_of(lo), grid.cell_of(hi));
        for (index, tri) in mesh.indices.iter().enumerate() {
            let points = tri.map(|v| mesh.positions[v as usize]);
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

    /// How far `p` is from the nearest triangle of `mesh`.
    ///
    /// Rings of cells around the point, outwards, until the nearest thing the
    /// next ring could possibly hold is further than the nearest thing already
    /// found. The ring bound is what keeps this cheap: a point sitting on the
    /// surface is answered by the first ring.
    fn distance(&self, mesh: &Mesh, p: Vec3) -> f64 {
        let mut best = f64::MAX;
        for &index in &self.everywhere {
            best = best.min(point_to_triangle(p, tri_points(mesh, index)));
        }
        let at = self.cell_of(p);
        let (lo, hi) = self.extent;
        // How far out there is anything to walk to: past this the rings are
        // entirely outside the mesh's own box.
        let reach = [(at.0, lo.0, hi.0), (at.1, lo.1, hi.1), (at.2, lo.2, hi.2)]
            .iter()
            .map(|&(c, lo, hi)| (c - lo).abs().max((c - hi).abs()))
            .max()
            .unwrap_or(0);
        for ring in 0..=reach {
            // Nothing outside this ring can be closer than its own walls, and
            // the ring before it has already been walked.
            if best <= f64::from(ring) * self.cell {
                return best;
            }
            for x in (at.0 - ring)..=(at.0 + ring) {
                for y in (at.1 - ring)..=(at.1 + ring) {
                    for z in (at.2 - ring)..=(at.2 + ring) {
                        // The shell of the ring only: the inside of it was
                        // walked by the ring before.
                        let shell = (x - at.0).abs() == ring || (y - at.1).abs() == ring || (z - at.2).abs() == ring;
                        if !shell {
                            continue;
                        }
                        let Some(bucket) = self.buckets.get(&(x, y, z)) else { continue };
                        for &index in bucket {
                            best = best.min(point_to_triangle(p, tri_points(mesh, index)));
                        }
                    }
                }
            }
        }
        best
    }
}

fn tri_points(mesh: &Mesh, index: u32) -> [Vec3; 3] {
    mesh.indices[index as usize].map(|v| mesh.positions[v as usize])
}

/// The distance from a point to a triangle: to its face where the point's
/// projection lands inside it, and to the nearest edge otherwise.
fn point_to_triangle(p: Vec3, tri: [Vec3; 3]) -> f64 {
    let [a, b, c] = tri;
    let normal = (b - a).cross(c - a);
    let area = normal.length();
    if area > 0.0 {
        let n = normal * (1.0 / area);
        let on_plane = p - n * (p - a).dot(n);
        // Inside when it is on the same side of all three edges, which the
        // cross products say without leaving the plane.
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

//! Every edge of a mesh with the triangles that meet at it.

use super::*;
use crate::vec3::Vec3;

/// A mesh's edges, each with its triangles: two inside a closed surface, one on a hole's rim, more
/// where bodies touch (a junction).
///
/// Edges are numbered in order of their lower vertex, then their upper one, so the edges leaving
/// a vertex upwards are one run and [`EdgeTable::find`] is a binary search.
#[derive(Clone, Debug, Default)]
pub struct EdgeTable {
    /// Each edge's ends, lower vertex first.
    ends: Vec<[u32; 2]>,
    /// The edges whose lower end is vertex `v` are `by_lower[v]..by_lower[v + 1]`.
    by_lower: Vec<u32>,
    /// Each edge's triangles, in ascending order.
    faces: Rows<u32>,
}

impl EdgeTable {
    pub fn of(mesh: &Mesh) -> EdgeTable {
        let lower = |tri: &[u32; 3], k: usize| {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            if a < b {
                (a, b)
            } else {
                (b, a)
            }
        };
        let vertices = mesh.positions.len();
        // Each triangle's sides under their lower vertex as (upper end, triangle), sorted so one
        // edge's triangles are one run.
        let mut sides = RowsBuilder::<(u32, u32)>::new(vertices);
        for tri in &mesh.indices {
            for k in 0..3 {
                sides.count(lower(tri, k).0 as usize);
            }
        }
        for (face, tri) in mesh.indices.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = lower(tri, k);
                sides.push(a as usize, (b, face as u32));
            }
        }
        let mut sides = sides.finish();
        for v in 0..vertices {
            sides.row_mut(v).sort_unstable();
        }
        // A closed surface has half as many edges as triangle sides; reserved so the arrays do not
        // regrow, which cost as much as the sort.
        let sides_total = sides.items().len();
        let mut ends = Vec::with_capacity(sides_total / 2);
        let mut face_start = Vec::with_capacity(sides_total / 2 + 1);
        let mut by_lower = Vec::with_capacity(vertices + 1);
        // The triangles stay in the sides' order, so each edge's are the run it was found in.
        let faces: Vec<u32> = sides.items().iter().map(|&(_, face)| face).collect();
        let mut at = 0u32;
        for a in 0..vertices {
            by_lower.push(ends.len() as u32);
            for run in sides.row(a).chunk_by(|x, y| x.0 == y.0) {
                ends.push([a as u32, run[0].0]);
                face_start.push(at);
                at += run.len() as u32;
            }
        }
        by_lower.push(ends.len() as u32);
        face_start.push(at);
        EdgeTable { ends, by_lower, faces: Rows::from_parts(face_start, faces) }
    }

    /// How many edges there are.
    pub fn len(&self) -> usize {
        self.ends.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    /// Edge `e`'s ends, lower vertex first.
    pub fn ends(&self, e: usize) -> [u32; 2] {
        self.ends[e]
    }

    /// The triangles meeting at edge `e`, in ascending order.
    pub fn faces(&self, e: usize) -> &[u32] {
        self.faces.row(e)
    }

    /// Each edge in order with its triangles.
    pub fn iter(&self) -> impl Iterator<Item = ([u32; 2], &[u32])> + '_ {
        self.ends.iter().enumerate().map(|(e, &ends)| (ends, self.faces.row(e)))
    }

    /// The edge between `a` and `b`, in either order, if the mesh has one.
    pub fn find(&self, a: u32, b: u32) -> Option<usize> {
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let first = *self.by_lower.get(lo as usize)? as usize;
        let last = self.by_lower[lo as usize + 1] as usize;
        let at = self.ends[first..last].binary_search_by_key(&hi, |ends| ends[1]).ok()?;
        Some(first + at)
    }

    /// The vertices sharing an edge with each of `vertices` vertices, in ascending order.
    pub fn neighbours(&self, vertices: usize) -> Rows<u32> {
        // Edges come in order of their lower end, so each row receives its lower neighbours in
        // ascending order before its upper ones, also ascending: sorted without a sort.
        let mut rows = RowsBuilder::new(vertices);
        for &[a, b] in &self.ends {
            rows.count(a as usize);
            rows.count(b as usize);
        }
        for &[a, b] in &self.ends {
            rows.push(a as usize, b);
            rows.push(b as usize, a);
        }
        rows.finish()
    }

    /// The edges worth drawing, and the two triangles beside each (one twice for a boundary edge, the
    /// first two at a junction), whose depth slope the edge's bias follows (issue 115).
    ///
    /// Real creases, where `normals` (one per triangle) turn by more than `angle_deg`, plus every
    /// boundary and junction: every triangle edge would just be noise.
    pub fn feature_edges(&self, normals: &[Vec3], angle_deg: f64) -> (Vec<[u32; 2]>, Vec<[u32; 2]>) {
        let cos_limit = angle_deg.to_radians().cos();
        self.feature_edges_by(normals, |_, _| cos_limit)
    }

    /// The same with a limit of its own per edge: `cos_limit(a, b)` is the cosine of the angle the
    /// triangles `a` and `b` beside it must turn by, so a smooth-shaded surface can keep its gentler
    /// turns unlined.
    pub fn feature_edges_by(
        &self,
        normals: &[Vec3],
        cos_limit: impl Fn(u32, u32) -> f64,
    ) -> (Vec<[u32; 2]>, Vec<[u32; 2]>) {
        let (mut edges, mut beside) = (Vec::new(), Vec::new());
        for (ends, faces) in self.iter() {
            let keep = match faces {
                [a, b] => normals[*a as usize].dot(normals[*b as usize]) < cos_limit(*a, *b),
                // A boundary edge or a non-manifold junction: both are worth seeing.
                _ => true,
            };
            if keep {
                edges.push(ends);
                beside.push([faces[0], *faces.get(1).unwrap_or(&faces[0])]);
            }
        }
        (edges, beside)
    }
}

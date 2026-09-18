//! The mesh in the form the collapses work on, and what may not be touched.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// A mesh being simplified: triangles that can be struck out, vertices that can
/// be moved, and, beside each vertex, everything the loop has to ask about it.
///
/// Triangles are never removed from the list, only marked dead, so an index
/// into it stays an index for the whole run and the adjacency does not have to
/// be renumbered on every collapse. The compacting happens once, at the end.
pub(crate) struct Surface {
    pub(crate) positions: Vec<Vec3>,
    pub(crate) tris: Vec<[u32; 3]>,
    pub(crate) tags: Vec<u32>,
    pub(crate) alive: Vec<bool>,
    /// The triangles that touch each vertex. Kept up to date through a
    /// collapse, and allowed to hold dead ones -- they are skipped where they
    /// are read, which is cheaper than hunting them down on every merge.
    pub(crate) incident: Vec<Vec<u32>>,
    pub(crate) quadrics: Vec<Quadric>,
    /// A vertex that may not move and may not be removed: it sits on something
    /// the settings say to keep.
    pub(crate) locked: Vec<bool>,
    /// The total area of the original triangles whose planes this vertex's
    /// quadric now stands for.
    ///
    /// What turns a quadric's error -- an area-weighted sum of squared
    /// distances, which grows as a vertex swallows its neighbours whether or
    /// not the surface has moved -- into a distance in millimetres that means
    /// the same thing at the start of a run and at the end of it. See
    /// [`super::collapse::deviation_of`].
    pub(crate) weight: Vec<f64>,
    pub(crate) live: usize,
}

/// How many triangles meet along one edge, and which they are.
type Edges = HashMap<(u32, u32), Vec<u32>>;

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

impl Surface {
    /// Read a mesh in, work out what is fixed, and give every vertex its
    /// quadric.
    pub(crate) fn build(mesh: &Mesh, plan: &Simplify) -> Surface {
        let welded = mesh.weld();
        let count = welded.positions.len();
        let tags = (0..welded.indices.len()).map(|i| welded.tag(i)).collect();
        let mut surface = Surface {
            positions: welded.positions,
            tags,
            alive: vec![true; welded.indices.len()],
            incident: vec![Vec::new(); count],
            quadrics: vec![Quadric::default(); count],
            locked: vec![false; count],
            weight: vec![0.0; count],
            live: welded.indices.len(),
            tris: welded.indices,
        };
        for (index, tri) in surface.tris.iter().enumerate() {
            for &v in tri {
                surface.incident[v as usize].push(index as u32);
            }
        }
        surface.accumulate_quadrics();
        surface.lock_features(plan);
        surface
    }

    /// Every triangle's plane, added to the three vertices that lie on it.
    fn accumulate_quadrics(&mut self) {
        for tri in &self.tris {
            let (a, b, c) =
                (self.positions[tri[0] as usize], self.positions[tri[1] as usize], self.positions[tri[2] as usize]);
            let cross = (b - a).cross(c - a);
            let area = cross.length() * 0.5;
            if area <= 0.0 {
                continue;
            }
            let quadric = Quadric::plane(cross * (1.0 / (area * 2.0)), a, area);
            for &v in tri {
                self.quadrics[v as usize].add(&quadric);
                self.weight[v as usize] += area;
            }
        }
    }

    /// Which edges the settings say to keep, turned into vertices that may not
    /// move.
    ///
    /// Keeping an *edge* by fixing both its ends is stricter than it has to be
    /// -- a run of collinear crease vertices could in principle be thinned
    /// along the crease without moving the crease -- and it is what the
    /// checkboxes promise: a corner stays exactly where it was, to the last
    /// bit, rather than staying within some tolerance of it. The cost is paid
    /// where features are dense, and a mesh that is *all* feature is one that
    /// has no detail to drop.
    fn lock_features(&mut self, plan: &Simplify) {
        let edges = self.edges();
        let sharp = plan.keep_sharp.then(|| plan.sharp_angle.to_radians().cos());
        let normals: Vec<Vec3> = self.tris.iter().map(|&tri| self.normal_of(tri)).collect();
        let mut fixed: Vec<(u32, u32)> = Vec::new();
        for (&(a, b), tris) in &edges {
            let keep = match tris.len() {
                // An edge with one triangle is a hole's rim: the mesh is not
                // closed there, and nothing on the far side holds the surface
                // in place.
                1 => plan.keep_boundaries,
                2 => {
                    let (one, two) = (normals[tris[0] as usize], normals[tris[1] as usize]);
                    let seam = plan.keep_colours && self.tags[tris[0] as usize] != self.tags[tris[1] as usize];
                    seam || sharp.is_some_and(|limit| one.dot(two) < limit)
                }
                // Three or more is where two bodies of one mesh meet -- the
                // seam between the cells of a split, where four faces share a
                // line. There is no single surface through it to simplify and
                // no way to tell what a collapse there would do to the other
                // bodies, so it is left exactly as it is, whatever the settings
                // say.
                _ => true,
            };
            if keep {
                fixed.push((a, b));
            }
        }
        for (a, b) in fixed {
            self.locked[a as usize] = true;
            self.locked[b as usize] = true;
        }
    }

    /// Every edge with the triangles along it. Built once, for the locking;
    /// the loop itself asks about one edge at a time, from the adjacency.
    fn edges(&self) -> Edges {
        let mut edges: Edges = HashMap::with_capacity(self.tris.len() * 2);
        for (index, tri) in self.tris.iter().enumerate() {
            for i in 0..3 {
                edges.entry(edge_key(tri[i], tri[(i + 1) % 3])).or_default().push(index as u32);
            }
        }
        edges
    }

    pub(crate) fn normal_of(&self, tri: [u32; 3]) -> Vec3 {
        let (a, b, c) =
            (self.positions[tri[0] as usize], self.positions[tri[1] as usize], self.positions[tri[2] as usize]);
        (b - a).cross(c - a).normalized()
    }

    /// The live triangles at a vertex, with the dead ones dropped from the list
    /// as they are found.
    pub(crate) fn tidy(&mut self, v: u32) {
        let alive = &self.alive;
        self.incident[v as usize].retain(|&t| alive[t as usize]);
    }

    /// The triangles along an edge, which is how the loop asks whether an edge
    /// is on the boundary without keeping a second index up to date.
    pub(crate) fn along(&self, a: u32, b: u32) -> Vec<u32> {
        self.incident[a as usize]
            .iter()
            .copied()
            .filter(|&t| self.alive[t as usize] && self.tris[t as usize].contains(&b))
            .collect()
    }

    /// The vertices joined to this one by a live triangle.
    pub(crate) fn neighbours(&self, v: u32) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        for &t in &self.incident[v as usize] {
            if !self.alive[t as usize] {
                continue;
            }
            for &other in &self.tris[t as usize] {
                if other != v && !out.contains(&other) {
                    out.push(other);
                }
            }
        }
        out
    }

    /// What is left, renumbered: the live triangles, the vertices they still
    /// use, and the tag each triangle came in with.
    pub(crate) fn finish(self) -> Mesh {
        let mut remap = vec![u32::MAX; self.positions.len()];
        let mut positions = Vec::new();
        let mut indices = Vec::with_capacity(self.live);
        let mut tags = Vec::with_capacity(self.live);
        for (index, tri) in self.tris.iter().enumerate() {
            if !self.alive[index] {
                continue;
            }
            let mut out = [0u32; 3];
            for (slot, &v) in out.iter_mut().zip(tri.iter()) {
                if remap[v as usize] == u32::MAX {
                    remap[v as usize] = positions.len() as u32;
                    positions.push(self.positions[v as usize]);
                }
                *slot = remap[v as usize];
            }
            indices.push(out);
            tags.push(self.tags[index]);
        }
        Mesh { positions, indices, tags }
    }
}

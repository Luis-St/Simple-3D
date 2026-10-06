//! The mesh in the form the collapses work on, and what may not be touched.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// A mesh being simplified, with per-vertex state. Triangles are only marked dead, never removed,
/// so indices stay valid; compaction happens once at the end.
pub(crate) struct Surface {
    pub(crate) positions: Vec<Vec3>,
    pub(crate) tris: Vec<[u32; 3]>,
    pub(crate) tags: Vec<u32>,
    pub(crate) alive: Vec<bool>,
    /// The triangles touching each vertex, possibly including dead ones, which readers skip.
    pub(crate) incident: Vec<Vec<u32>>,
    pub(crate) quadrics: Vec<Quadric>,
    /// A vertex that may not move or be removed, because the settings keep what it sits on.
    pub(crate) locked: Vec<bool>,
    /// The original area this vertex's quadric stands for, turning its error into a scale-independent
    /// distance ([`super::collapse::deviation_of`]).
    pub(crate) weight: Vec<f64>,
    pub(crate) live: usize,
}

/// The triangles meeting along each edge.
type Edges = HashMap<(u32, u32), Vec<u32>>;

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

impl Surface {
    /// Read a mesh in, lock what is fixed, and give every vertex its quadric.
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

    /// Add every triangle's plane to its three vertices' quadrics.
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

    /// Lock the vertices of edges the settings keep. Fixing both ends is stricter than needed but is
    /// what the settings promise: a kept corner stays exactly in place.
    fn lock_features(&mut self, plan: &Simplify) {
        let edges = self.edges();
        let sharp = plan.keep_sharp.then(|| plan.sharp_angle.to_radians().cos());
        let normals: Vec<Vec3> = self.tris.iter().map(|&tri| self.normal_of(tri)).collect();
        let mut fixed: Vec<(u32, u32)> = Vec::new();
        for (&(a, b), tris) in &edges {
            let keep = match tris.len() {
                // One triangle: a hole's rim, held by nothing on the far side.
                1 => plan.keep_boundaries,
                2 => {
                    let (one, two) = (normals[tris[0] as usize], normals[tris[1] as usize]);
                    let seam = plan.keep_colours && self.tags[tris[0] as usize] != self.tags[tris[1] as usize];
                    seam || sharp.is_some_and(|limit| one.dot(two) < limit)
                }
                // Three or more: where bodies of one mesh meet (split seams). Always kept, whatever the settings.
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

    /// Every edge with its triangles, built once for locking.
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

    /// Drop dead triangles from a vertex's incident list.
    pub(crate) fn tidy(&mut self, v: u32) {
        let alive = &self.alive;
        self.incident[v as usize].retain(|&t| alive[t as usize]);
    }

    /// The triangles along an edge, from the adjacency.
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

    /// The result, renumbered: live triangles, their vertices, and their original tags.
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
        Mesh { positions, indices, tags, sources: Vec::new() }
    }
}

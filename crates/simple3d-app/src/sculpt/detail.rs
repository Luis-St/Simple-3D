//! Adding detail under a dab (issue 127), and what the brush may move.
//!
//! Adding detail splits triangles, so it is handled dab by dab: a dab that finds edges longer than
//! asked under it splits them first and makes the copy's caches again, which costs about what
//! starting a session does. Once an area is fine enough, dabs there find nothing to split and cost
//! nothing extra; the meshes that need detail are the coarse ones, which are quick to remake.

use super::*;
use simple3d_geom::brush::refine_within;
use simple3d_geom::shaping::{weights, Shaping, MAX_TRIANGLES};
use simple3d_geom::topology::vertex_normals;
use simple3d_geom::Mesh;
use std::collections::BTreeMap;

impl Sculpt {
    /// Weigh each vertex for the stroke by `keeping` (a hole's rim, a print's base), measured in the
    /// mesh's own frame as the mesh functions measure it.
    pub fn weigh(&mut self, keeping: &Shaping) {
        let inverse = self.placement.inverse();
        let world = &self.renderable.mesh;
        let local = Mesh {
            positions: world.positions.iter().map(|&p| inverse.point(p)).collect(),
            indices: Vec::new(),
            tags: Vec::new(),
            sources: Vec::new(),
        };
        let weights = weights(&local, &self.topology, keeping);
        if let Some(stroke) = self.stroke.as_mut() {
            stroke.weights = weights;
        }
    }

    /// Split the edges under a dab at `centre` longer than `edge`. Whether anything was split, in which
    /// case the copy and its caches were made again and the stroke's weights must be too.
    pub fn add_detail(&mut self, start: usize, centre: Vec3, radius: f64, edge: f64) -> bool {
        if self.stroke.is_none() || edge.is_nan() || edge <= 0.0 || !self.needs_detail(start, centre, radius, edge) {
            return false;
        }
        let mut world = self.renderable.mesh.clone();
        // The stored tags ride along as sources, which splitting hands to each piece as it does tags.
        world.sources = self.tags.clone();
        let Some((mut refined, parents)) = refine_within(&world, edge, centre, radius, MAX_TRIANGLES) else {
            return false;
        };
        let rests = self.rests_between(&parents);
        self.tags = std::mem::take(&mut refined.sources);
        self.reshape(refined, rests);
        true
    }

    /// Where each new vertex (halfway between `parents`, in order) was when the stroke began, and
    /// facing which way: halfway between where its two vertices were. `None` for one between vertices
    /// the stroke has not moved, which starts where it is.
    fn rests_between(&self, parents: &[[u32; 2]]) -> Vec<Option<(Vec3, Vec3)>> {
        let Some(stroke) = self.stroke.as_ref() else { return Vec::new() };
        let (positions, old) = (&self.renderable.mesh.positions, self.renderable.mesh.positions.len());
        // Each new vertex's place and normal then, and whether the stroke had moved either of its two.
        let mut found: Vec<((Vec3, Vec3), bool)> = Vec::with_capacity(parents.len());
        for &[a, b] in parents {
            let [((pa, na), ma), ((pb, nb), mb)] = [a, b].map(|v| match (v as usize).checked_sub(old) {
                Some(k) => found[k],
                None => {
                    stroke.rest(v).map_or(((positions[v as usize], self.normals[v as usize]), false), |r| (r, true))
                }
            });
            found.push((((pa + pb) * 0.5, (na + nb).normalized()), ma || mb));
        }
        found.into_iter().map(|(rest, moved)| moved.then_some(rest)).collect()
    }

    /// Whether a triangle the dab reaches has a side too long: cheaper than trying to refine, which
    /// copies the whole mesh.
    fn needs_detail(&mut self, start: usize, centre: Vec3, radius: f64, edge: f64) -> bool {
        let reached = self.within(start, centre, radius);
        let mesh = &self.renderable.mesh;
        let faces = reached.iter().flat_map(|&(v, _)| self.topology.faces.row(v as usize).iter().copied());
        std::iter::once(start as u32).chain(faces).any(|face| {
            let [a, b, c] = mesh.corners(mesh.indices[face as usize]);
            [(a, b), (b, c), (c, a)].iter().any(|&(from, to)| (to - from).length() > edge)
        })
    }

    /// Make the copy's caches again for `mesh`, whose triangles changed: the old vertices keep their
    /// numbers, the new ones follow with where they were when the stroke began (`rests`). The stroke
    /// keeps where it found the old ones, or the noise mode would measure from where they now are and
    /// pile up with every dab that adds detail.
    fn reshape(&mut self, mesh: Mesh, rests: Vec<Option<(Vec3, Vec3)>>) {
        let count = mesh.indices.len();
        self.smooth = vec![true; count];
        self.topology = Topology::of(&mesh);
        self.normals = vertex_normals(&mesh);
        self.tree = Bvh::build(&mesh);
        self.refit = self.tree.refit_map();
        self.renderable = Renderable::prepare_welded(mesh, true, &self.smooth, BTreeMap::new());
        self.visited.clear();
        if let Some(stroke) = self.stroke.as_mut() {
            let old = stroke.seen.len();
            stroke.seen.resize(self.renderable.mesh.positions.len(), u32::MAX);
            for (k, rest) in rests.into_iter().enumerate() {
                if let Some((position, normal)) = rest {
                    stroke.seen[old + k] = stroke.before.len() as u32;
                    stroke.before.push(((old + k) as u32, position, normal));
                }
            }
            stroke.reshaped = true;
        }
    }
}

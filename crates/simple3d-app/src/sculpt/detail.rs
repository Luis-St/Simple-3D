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
        let Some(mut refined) = refine_within(&world, edge, centre, radius, MAX_TRIANGLES) else { return false };
        self.tags = std::mem::take(&mut refined.sources);
        self.reshape(refined);
        true
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

    /// Make the copy's caches again for `mesh`, whose triangles changed.
    fn reshape(&mut self, mesh: Mesh) {
        let count = mesh.indices.len();
        self.smooth = vec![true; count];
        self.topology = Topology::of(&mesh);
        self.normals = vertex_normals(&mesh);
        self.tree = Bvh::build(&mesh);
        self.refit = self.tree.refit_map();
        self.renderable = Renderable::prepare_welded(mesh, true, &self.smooth, BTreeMap::new());
        self.visited.clear();
        if let Some(stroke) = self.stroke.as_mut() {
            stroke.seen = vec![u32::MAX; self.renderable.mesh.positions.len()];
            stroke.before.clear();
            stroke.reshaped = true;
        }
    }
}

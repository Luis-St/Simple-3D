//! Moving vertices of the working copy and keeping what depends on them current: the triangle,
//! vertex and corner normals around them, the picking tree, and the list of changes for the card.

use super::*;
use crate::render::SMOOTH_CREASE;
use simple3d_geom::topology::{corner_normals_at, vertex_normals_at};

impl Sculpt {
    /// Start a stroke. The copy gets a new renderable id, so the card takes it whole once rather
    /// than trusting what an earlier stroke left there.
    pub fn begin(&mut self) {
        self.renderable.renew_id();
        self.edits.sent();
        self.stroke = Some(Stroke { seen: vec![false; self.renderable.mesh.positions.len()], before: Vec::new() });
        self.settling = false;
    }

    /// Move each vertex to its new world position: one dab. Outside a stroke nothing moves.
    pub fn move_vertices(&mut self, moves: &[(u32, Vec3)]) {
        let Some(stroke) = self.stroke.as_mut() else { return };
        let positions = &mut self.renderable.mesh.positions;
        for &(v, to) in moves {
            if !std::mem::replace(&mut stroke.seen[v as usize], true) {
                stroke.before.push((v, positions[v as usize]));
            }
            positions[v as usize] = to;
        }
        let mut moved: Vec<u32> = moves.iter().map(|&(v, _)| v).collect();
        moved.sort_unstable();
        moved.dedup();
        self.refresh(&moved);
    }

    /// Put every vertex the stroke moved back where it was, and end the stroke.
    pub fn restore(&mut self) {
        let Some(stroke) = self.stroke.take() else { return };
        let mut moved = Vec::with_capacity(stroke.before.len());
        for &(v, at) in &stroke.before {
            self.renderable.mesh.positions[v as usize] = at;
            moved.push(v);
        }
        moved.sort_unstable();
        self.refresh(&moved);
    }

    /// Bring everything that depends on the `moved` vertices (sorted, no repeats) up to date.
    fn refresh(&mut self, moved: &[u32]) {
        if moved.is_empty() {
            return;
        }
        let topology = &self.topology;
        let gather = |from: &[u32], row: &dyn Fn(u32) -> Vec<u32>| {
            let mut out: Vec<u32> = from.iter().flat_map(|&at| row(at)).collect();
            out.sort_unstable();
            out.dedup();
            out
        };
        let mesh = &self.renderable.mesh;
        // The triangles that turned, every vertex of theirs (whose normals and corners change with
        // them), and those vertices' triangles, whose corner normals are rewritten.
        let faces = gather(moved, &|v| topology.faces.row(v as usize).to_vec());
        let ring = gather(&faces, &|f| mesh.indices[f as usize].to_vec());
        let around = gather(&ring, &|v| topology.faces.row(v as usize).to_vec());
        for &face in &faces {
            self.renderable.normals[face as usize] = mesh.triangle_normal(mesh.indices[face as usize]);
        }
        vertex_normals_at(mesh, &topology.faces, &ring, &mut self.normals);
        let (edges, rows) = (&topology.edges, &topology.faces);
        let corners = &mut self.renderable.corner_normals;
        corner_normals_at(mesh, edges, rows, SMOOTH_CREASE, &self.smooth, &ring, corners);
        let reach = moved.iter().map(|&v| mesh.positions[v as usize].length()).fold(self.renderable.reach, f64::max);
        self.renderable.reach = reach;
        self.tree.refit(&self.refit, mesh, &faces);
        self.renderable.forget_plane_marks();
        self.edits.mark(moved, &around);
    }
}

//! Which vertices a dab reaches: those within its radius of where it lands, connected to it over
//! the surface, so a dab on one side of a thin wall does not reach through to the other.

use super::*;
use std::collections::VecDeque;

impl Sculpt {
    /// The vertices within `radius` of `centre`, flooding out over edges from triangle `start`'s
    /// corners, with their distances. Visit marks are kept between calls, so a dab costs only what it
    /// reaches.
    pub fn within(&mut self, start: usize, centre: Vec3, radius: f64) -> Vec<(u32, f64)> {
        let positions = &self.renderable.mesh.positions;
        if self.visited.len() != positions.len() {
            self.visited = vec![0; positions.len()];
        }
        self.visit = self.visit.wrapping_add(1);
        if self.visit == 0 {
            self.visited.iter_mut().for_each(|mark| *mark = 0);
            self.visit = 1;
        }
        let mark = self.visit;
        let mut out = Vec::new();
        let mut queue: VecDeque<u32> = VecDeque::new();
        let Some(tri) = self.renderable.mesh.indices.get(start) else { return out };
        for &v in tri {
            self.visited[v as usize] = mark;
            queue.push_back(v);
        }
        while let Some(v) = queue.pop_front() {
            let d = (positions[v as usize] - centre).length();
            // The hit triangle's corners always count as reached, so even a dab smaller than a
            // triangle has somewhere to start from.
            if d > radius && !tri.contains(&v) {
                continue;
            }
            if d <= radius {
                out.push((v, d));
            }
            for &n in self.topology.neighbours.row(v as usize) {
                if self.visited[n as usize] != mark {
                    self.visited[n as usize] = mark;
                    queue.push_back(n);
                }
            }
        }
        out
    }
}

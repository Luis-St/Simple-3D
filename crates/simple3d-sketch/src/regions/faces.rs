//! Tracing the graph's faces: every loop of half-edges keeping one face on its left.
//!
//! Half-edge `2k` runs along edge `k` from `u` to `v`, `2k + 1` back. Leaving a vertex, the next
//! half-edge of a face is the one just clockwise of the way back, so bounded faces come out
//! counter-clockwise (positive area) and the outside of each connected piece clockwise. Edges that
//! bound nothing -- dangling ends, and bridges with the same face on both sides -- are dropped first.

use super::graph::Graph;

/// The graph with its loops traced.
pub(crate) struct Faces {
    pub graph: Graph,
    /// Which edges survived the clean-up.
    pub alive: Vec<bool>,
    /// The outgoing half-edges at each vertex, counter-clockwise by angle.
    pub around: Vec<Vec<usize>>,
    /// Every loop, as half-edges.
    pub loops: Vec<Vec<usize>>,
}

impl Faces {
    pub fn origin(&self, h: usize) -> usize {
        let edge = self.graph.edges[h / 2];
        if h.is_multiple_of(2) {
            edge.u
        } else {
            edge.v
        }
    }

    pub fn target(&self, h: usize) -> usize {
        self.origin(h ^ 1)
    }

    /// The half-edge after `h` keeping the same face on the left, among those `allowed`.
    pub fn next(&self, h: usize, allowed: impl Fn(usize) -> bool) -> Option<usize> {
        let at = &self.around[self.target(h)];
        let back = at.iter().position(|&k| k == h ^ 1)?;
        (1..=at.len()).map(|step| at[(back + at.len() - step) % at.len()]).find(|&k| allowed(k))
    }

    /// The loop's corners.
    pub fn points(&self, path: &[usize]) -> Vec<[f64; 2]> {
        path.iter().map(|&h| self.graph.pos[self.origin(h)]).collect()
    }
}

/// The shoelace area, positive counter-clockwise.
pub(crate) fn signed_area(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    (0..n).map(|i| crate::cross(points[i], points[(i + 1) % n])).sum::<f64>() * 0.5
}

/// Whether `p` is inside the polygon `points`, by crossings.
pub(crate) fn contains(points: &[[f64; 2]], p: [f64; 2]) -> bool {
    let n = points.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Trace `graph`'s loops, dropping what bounds nothing until nothing more goes.
pub(crate) fn trace(graph: Graph) -> Faces {
    let mut faces = Faces { alive: vec![true; graph.edges.len()], graph, around: Vec::new(), loops: Vec::new() };
    loop {
        drop_dangling(&mut faces);
        sort_around(&mut faces);
        trace_loops(&mut faces);
        // A bridge is walked twice by one loop; with it gone that loop splits into the two it joined.
        let mut owner = vec![usize::MAX; faces.graph.edges.len() * 2];
        for (index, path) in faces.loops.iter().enumerate() {
            for &h in path {
                owner[h] = index;
            }
        }
        let mut bridged = false;
        for k in 0..faces.graph.edges.len() {
            if faces.alive[k] && owner[2 * k] == owner[2 * k + 1] {
                faces.alive[k] = false;
                bridged = true;
            }
        }
        if !bridged {
            return faces;
        }
    }
}

fn drop_dangling(faces: &mut Faces) {
    let count = faces.graph.pos.len();
    loop {
        let mut degree = vec![0usize; count];
        for (edge, _) in faces.graph.edges.iter().zip(&faces.alive).filter(|(_, alive)| **alive) {
            degree[edge.u] += 1;
            degree[edge.v] += 1;
        }
        let mut dropped = false;
        for (k, edge) in faces.graph.edges.iter().enumerate() {
            if faces.alive[k] && (degree[edge.u] < 2 || degree[edge.v] < 2) {
                faces.alive[k] = false;
                dropped = true;
            }
        }
        if !dropped {
            return;
        }
    }
}

fn sort_around(faces: &mut Faces) {
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); faces.graph.pos.len()];
    for k in (0..faces.graph.edges.len()).filter(|&k| faces.alive[k]) {
        around[faces.graph.edges[k].u].push(2 * k);
        around[faces.graph.edges[k].v].push(2 * k + 1);
    }
    for (vertex, list) in around.iter_mut().enumerate() {
        let from = faces.graph.pos[vertex];
        let angle = |h: usize| {
            let edge = faces.graph.edges[h / 2];
            let to = faces.graph.pos[if h.is_multiple_of(2) { edge.v } else { edge.u }];
            (to[1] - from[1]).atan2(to[0] - from[0])
        };
        list.sort_by(|&a, &b| angle(a).total_cmp(&angle(b)));
    }
    faces.around = around;
}

fn trace_loops(faces: &mut Faces) {
    let mut used = vec![false; faces.graph.edges.len() * 2];
    let mut loops = Vec::new();
    for start in 0..faces.graph.edges.len() * 2 {
        if used[start] || !faces.alive[start / 2] {
            continue;
        }
        let mut path = Vec::new();
        let mut h = start;
        while !used[h] {
            used[h] = true;
            path.push(h);
            match faces.next(h, |_| true) {
                Some(next) => h = next,
                None => break,
            }
        }
        loops.push(path);
    }
    faces.loops = loops;
}

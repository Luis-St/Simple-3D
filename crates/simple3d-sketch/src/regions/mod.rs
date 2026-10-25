//! The closed areas a sketch's curves enclose, which the user picks from (issue 74).
//!
//! The curves are split wherever they cross ([`graph`]) and the faces of the resulting planar graph
//! traced ([`faces`]). Each bounded face is a region; a separate piece of drawing lying inside one is
//! a hole in it, so a circle inside a rectangle makes a ring and a disc. A pick is remembered by the
//! sides of the curves around the region ([`RegionPick`]), and found again after edits by those.

mod graph;
mod faces;
pub(crate) use faces::{contains, signed_area};
mod profile;

use crate::curve::CurveId;
use crate::sketch::{RegionPick, Sketch};
use faces::Faces;

/// One closed area.
#[derive(Clone, Debug)]
pub struct Region {
    /// Its outer edge, counter-clockwise.
    pub outer: Vec<[f64; 2]>,
    /// The edges of what lies inside it, clockwise.
    pub holes: Vec<Vec<[f64; 2]>>,
    /// The curve sides around it, as [`RegionPick::sides`] has them.
    pub sides: Vec<(CurveId, bool)>,
    /// Its area, holes taken off.
    pub area: f64,
    /// Its loops in the traced graph, outer first.
    loops: Vec<usize>,
}

impl Region {
    /// Whether `p` lies inside it and not in one of its holes.
    pub fn contains(&self, p: [f64; 2]) -> bool {
        contains(&self.outer, p) && !self.holes.iter().any(|hole| contains(hole, p))
    }

    /// The middle of its outer edge's corners, for telling regions with the same sides apart.
    fn middle(&self) -> [f64; 2] {
        let sum = self.outer.iter().fold([0.0; 2], |s, p| crate::add(s, *p));
        crate::scale(sum, 1.0 / self.outer.len().max(1) as f64)
    }
}

/// Every region of a sketch, over the traced graph they came from.
pub struct Regions {
    faces: Faces,
    pub list: Vec<Region>,
}

/// The regions of `sketch`, with curves sampled at `segments` to a full turn.
pub fn find(sketch: &Sketch, segments: u32) -> Regions {
    let faces = faces::trace(graph::build(sketch, segments));
    // Which connected piece each vertex is in, by union-find over the surviving edges.
    let mut parent: Vec<usize> = (0..faces.graph.pos.len()).collect();
    fn root(parent: &mut [usize], mut k: usize) -> usize {
        while parent[k] != k {
            parent[k] = parent[parent[k]];
            k = parent[k];
        }
        k
    }
    for (k, edge) in faces.graph.edges.iter().enumerate() {
        if faces.alive[k] {
            let (a, b) = (root(&mut parent, edge.u), root(&mut parent, edge.v));
            parent[a] = b;
        }
    }
    let points: Vec<Vec<[f64; 2]>> = faces.loops.iter().map(|path| faces.points(path)).collect();
    let areas: Vec<f64> = points.iter().map(|p| signed_area(p)).collect();
    let piece: Vec<usize> = faces.loops.iter().map(|path| root(&mut parent, faces.origin(path[0]))).collect();
    let mut list: Vec<Region> = Vec::new();
    let mut of_loop = vec![usize::MAX; faces.loops.len()];
    for index in 0..faces.loops.len() {
        if areas[index] <= 0.0 {
            continue;
        }
        of_loop[index] = list.len();
        list.push(Region {
            outer: points[index].clone(),
            holes: Vec::new(),
            sides: Vec::new(),
            area: areas[index],
            loops: vec![index],
        });
    }
    // Each piece's outside is a hole in the smallest region of another piece around it.
    for (index, area) in areas.iter().enumerate() {
        if *area > 0.0 {
            continue;
        }
        let test = points[index][0];
        let around = (0..faces.loops.len())
            .filter(|&k| areas[k] > 0.0 && piece[k] != piece[index] && contains(&points[k], test))
            .min_by(|&a, &b| areas[a].total_cmp(&areas[b]));
        if let Some(container) = around {
            let region = &mut list[of_loop[container]];
            region.holes.push(points[index].clone());
            region.area += area;
            region.loops.push(index);
        }
    }
    for region in &mut list {
        let mut sides: Vec<(CurveId, bool)> = region
            .loops
            .iter()
            .flat_map(|&l| faces.loops[l].iter())
            .map(|&h| {
                let edge = faces.graph.edges[h / 2];
                (edge.curve, h.is_multiple_of(2) == edge.forward)
            })
            .collect();
        sides.sort_unstable();
        sides.dedup();
        region.sides = sides;
    }
    Regions { faces, list }
}

impl Regions {
    /// The smallest region `p` lies in.
    pub fn at(&self, p: [f64; 2]) -> Option<usize> {
        (0..self.list.len()).filter(|&k| self.list[k].contains(p)).min_by(|&a, &b| {
            self.list[a].area.total_cmp(&self.list[b].area)
        })
    }

    /// How to remember region `index`.
    pub fn pick(&self, index: usize, hint: [f64; 2]) -> RegionPick {
        RegionPick { sides: self.list[index].sides.clone(), hint }
    }

    /// The region `pick` stands for now: the one with the same curve sides, else the one sharing
    /// most of them (at least half), with the hint settling ties. `None` once the curves around it
    /// are mostly gone.
    pub fn resolve(&self, pick: &RegionPick) -> Option<usize> {
        let score = |region: &Region| {
            let shared = region.sides.iter().filter(|side| pick.sides.contains(side)).count();
            let union = region.sides.len() + pick.sides.len() - shared;
            shared as f64 / union.max(1) as f64
        };
        let mut best: Option<(usize, f64, bool, f64)> = None;
        for (k, region) in self.list.iter().enumerate() {
            let s = score(region);
            if s < 0.5 {
                continue;
            }
            let holds = region.contains(pick.hint);
            let gap = crate::distance(region.middle(), pick.hint);
            let better = match best {
                None => true,
                Some((_, bs, bh, bg)) => {
                    s > bs + 1e-9 || ((s - bs).abs() <= 1e-9 && (holds && !bh || holds == bh && gap < bg))
                }
            };
            if better {
                best = Some((k, s, holds, gap));
            }
        }
        best.map(|b| b.0)
    }

    /// Where each of `picks` is now, skipping the ones that are gone; without repeats.
    pub fn resolve_all(&self, picks: &[RegionPick]) -> Vec<usize> {
        let mut out: Vec<usize> = picks.iter().filter_map(|pick| self.resolve(pick)).collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

//! Picked regions merged into the outlines a solid is made from: neighbouring picks become one
//! outline, with the edges between them gone.

use super::faces::{contains, signed_area};
use super::Regions;
use simple3d_geom::push_pull::Outline;
use std::collections::HashSet;

impl Regions {
    /// The outlines of regions `picked` taken together. Every half-edge of a picked region whose twin
    /// is not also picked is on the edge of the whole; those are traced into loops again, the
    /// counter-clockwise ones outer edges and the rest holes in the smallest outer edge around them.
    pub fn outlines(&self, picked: &[usize]) -> Vec<Outline> {
        let faces = &self.faces;
        let chosen: HashSet<usize> = picked
            .iter()
            .filter_map(|&k| self.list.get(k))
            .flat_map(|region| region.loops.iter().flat_map(|&l| faces.loops[l].iter().copied()))
            .collect();
        let edge: HashSet<usize> = chosen.iter().copied().filter(|h| !chosen.contains(&(h ^ 1))).collect();
        let mut used: HashSet<usize> = HashSet::new();
        let mut loops: Vec<Vec<[f64; 2]>> = Vec::new();
        let mut sorted: Vec<usize> = edge.iter().copied().collect();
        sorted.sort_unstable();
        for start in sorted {
            if used.contains(&start) {
                continue;
            }
            let mut path = Vec::new();
            let mut h = start;
            while used.insert(h) {
                path.push(h);
                match faces.next(h, |k| edge.contains(&k)) {
                    Some(next) => h = next,
                    None => break,
                }
            }
            if path.len() >= 3 {
                loops.push(faces.points(&path));
            }
        }
        let areas: Vec<f64> = loops.iter().map(|l| signed_area(l)).collect();
        let mut holes: Vec<Vec<Vec<[f64; 2]>>> = vec![Vec::new(); loops.len()];
        for (index, hole) in loops.iter().enumerate() {
            if areas[index] > 0.0 {
                continue;
            }
            let around = (0..loops.len())
                .filter(|&k| areas[k] > 0.0 && contains(&loops[k], hole[0]))
                .min_by(|&a, &b| areas[a].total_cmp(&areas[b]));
            if let Some(outer) = around {
                holes[outer].push(hole.clone());
            }
        }
        (0..loops.len())
            .filter(|&k| areas[k] > 0.0)
            .filter_map(|k| Outline::new(loops[k].clone(), std::mem::take(&mut holes[k])))
            .collect()
    }
}

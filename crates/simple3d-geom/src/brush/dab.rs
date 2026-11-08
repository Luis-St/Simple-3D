//! One dab: where each vertex under the brush moves.

use super::*;
use crate::shaping::Noise;
use crate::topology::Rows;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// The surface as a dab reads it: positions, unit vertex normals, each vertex's neighbours, and how
/// strongly each may move (`crate::shaping::weights`).
pub struct Surface<'a> {
    pub positions: &'a [Vec3],
    pub normals: &'a [Vec3],
    pub neighbours: &'a Rows<u32>,
    pub weights: &'a [f64],
    /// Where a vertex was when the stroke began, and its normal then, if the stroke has moved it:
    /// the noise mode measures its height from there, so going over a spot again does not pile up.
    pub rest: &'a dyn Fn(u32) -> Option<(Vec3, Vec3)>,
}

/// The new positions of the vertices in `region` (each with its distance from the dab's centre)
/// after one dab of `brush` in `mode`. Vertices that would not move are left out.
pub fn dab(brush: &Brush, mode: Mode, surface: &Surface, region: &[(u32, f64)]) -> Vec<(u32, Vec3)> {
    let share = |v: u32, d: f64| brush.strength * brush.falloff.at(d, brush.radius) * surface.weights[v as usize];
    let lift = brush.radius * 0.1;
    // Raise and lower move every vertex along the surface's mean normal under the brush, weighted by
    // the falloff, as a sculpting program's draw brush does. Along each vertex's own normal the
    // vertices on a ridge's flanks splay apart, pinching its crest into a crease, and where many thin
    // triangles meet (a converted sphere's pole) the normals fan out and tore the surface.
    let area = region
        .iter()
        .fold(Vec3::ZERO, |sum, &(v, d)| sum + surface.normals[v as usize] * brush.falloff.at(d, brush.radius))
        .normalized();
    let along = |sign: f64| -> Vec<(u32, Vec3)> {
        region
            .iter()
            .filter_map(|&(v, d)| {
                let p = surface.positions[v as usize];
                let step = sign * lift * share(v, d);
                (step != 0.0).then(|| (v, p + area * step))
            })
            .collect()
    };
    match mode {
        Mode::Raise => along(1.0),
        Mode::Lower => along(-1.0),
        Mode::Noise => noise(brush, surface, region, &share),
        Mode::Smooth => smooth(surface, region, &share),
        Mode::Flatten => flatten(brush, surface, region, &share),
    }
}

/// Towards the noise pattern's height over where each vertex was when the stroke began, at most the
/// strength's share of a tenth of the radius either way, as a raise lifts: each dab goes half its
/// falloff's share of the way there, so the pattern grows in where the brush goes but never past its
/// height.
fn noise(brush: &Brush, surface: &Surface, region: &[(u32, f64)], share: &dyn Fn(u32, f64) -> f64) -> Vec<(u32, Vec3)> {
    let noise = Noise::new(brush.seed);
    let scale = brush.scale.max(1e-6);
    let height = brush.radius * 0.1 * brush.strength;
    region
        .iter()
        .filter_map(|&(v, d)| {
            let p = surface.positions[v as usize];
            let (from, normal) = (surface.rest)(v).unwrap_or((p, surface.normals[v as usize]));
            // Perlin noise stays within about half its range; doubled, it reaches the full height.
            let level = (noise.layered(from / scale, 3, Noise::perlin) * 2.0).clamp(-1.0, 1.0);
            let target = from + normal * (height * level);
            let amount = 0.5 * share(v, d) / brush.strength.max(1e-12);
            (amount != 0.0).then(|| (v, p + (target - p) * amount))
        })
        .collect()
}

/// Taubin's pull and push, as `crate::shaping` smooths a whole mesh, over the region only: the
/// second step reads the first's results, and vertices outside stay where they are.
fn smooth(surface: &Surface, region: &[(u32, f64)], share: &dyn Fn(u32, f64) -> f64) -> Vec<(u32, Vec3)> {
    const PULL: f64 = 0.5;
    const PUSH: f64 = -0.53;
    let mut moved: HashMap<u32, Vec3> = HashMap::with_capacity(region.len());
    for factor in [PULL, PUSH] {
        let at = |v: u32, moved: &HashMap<u32, Vec3>| moved.get(&v).copied().unwrap_or(surface.positions[v as usize]);
        let next: Vec<(u32, Vec3)> = region
            .iter()
            .filter_map(|&(v, d)| {
                let ring = surface.neighbours.row(v as usize);
                let amount = share(v, d);
                if ring.is_empty() || amount == 0.0 {
                    return None;
                }
                let middle = ring.iter().fold(Vec3::ZERO, |sum, &n| sum + at(n, &moved)) / ring.len() as f64;
                let p = at(v, &moved);
                Some((v, p + (middle - p) * (factor * amount)))
            })
            .collect();
        moved.extend(next);
    }
    region.iter().filter_map(|&(v, _)| moved.get(&v).map(|&p| (v, p))).collect()
}

/// Towards the plane through the region's falloff-weighted centre, facing along its weighted normal.
fn flatten(
    brush: &Brush,
    surface: &Surface,
    region: &[(u32, f64)],
    share: &dyn Fn(u32, f64) -> f64,
) -> Vec<(u32, Vec3)> {
    let (mut sum, mut facing, mut total) = (Vec3::ZERO, Vec3::ZERO, 0.0);
    for &(v, d) in region {
        let w = brush.falloff.at(d, brush.radius);
        sum = sum + surface.positions[v as usize] * w;
        facing = facing + surface.normals[v as usize] * w;
        total += w;
    }
    let normal = facing.normalized();
    if total <= 0.0 || normal == Vec3::ZERO {
        return Vec::new();
    }
    let middle = sum / total;
    region
        .iter()
        .filter_map(|&(v, d)| {
            let p = surface.positions[v as usize];
            let amount = share(v, d);
            (amount != 0.0).then(|| (v, p - normal * ((p - middle).dot(normal) * amount)))
        })
        .collect()
}

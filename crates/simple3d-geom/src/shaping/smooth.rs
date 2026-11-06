//! Smoothing a surface without shrinking it.

use crate::topology::Rows;
use crate::vec3::Vec3;
use crate::Abandon;

/// Taubin's two-step smoothing: each pass pulls every vertex towards its neighbours' centre, then
/// pushes it back out slightly further, which takes out bumps while the volume stays put. Plain
/// averaging shrinks a closed shape a little with every pass. `weights` scale each vertex's steps;
/// `None` when `give_up` says so.
pub(super) fn taubin(
    positions: &mut [Vec3],
    neighbours: &Rows<u32>,
    weights: &[f64],
    passes: u32,
    give_up: Abandon<'_>,
) -> Option<()> {
    // Taubin's values: a pass band just above the mesh's own sampling, so features survive.
    const PULL: f64 = 0.5;
    const PUSH: f64 = -0.53;
    let mut next = positions.to_vec();
    for _ in 0..passes {
        for factor in [PULL, PUSH] {
            if give_up() {
                return None;
            }
            for (v, out) in next.iter_mut().enumerate() {
                let ring = neighbours.row(v);
                if ring.is_empty() || weights[v] == 0.0 {
                    *out = positions[v];
                    continue;
                }
                let centre = ring.iter().fold(Vec3::ZERO, |sum, &n| sum + positions[n as usize]) / ring.len() as f64;
                *out = positions[v] + (centre - positions[v]) * (factor * weights[v]);
            }
            positions.copy_from_slice(&next);
        }
    }
    Some(())
}

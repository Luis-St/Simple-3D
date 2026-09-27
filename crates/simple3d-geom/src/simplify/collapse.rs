//! Choosing the next edge to collapse, checking it is safe, and doing it.

use super::*;
use crate::vec3::Vec3;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A queued edge with its cost at queue time.
///
/// Costs go stale when a neighbouring collapse changes a quadric. Instead of an index to fix
/// them, each entry records its ends' touch counts; stale entries are discarded when popped,
/// since the edge was already re-queued at its new price.
struct Candidate {
    cost: f64,
    ends: [u32; 2],
    stamps: [u32; 2],
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Candidate) -> bool {
        self.cost == other.cost
    }
}

impl Eq for Candidate {}

impl Ord for Candidate {
    /// Reversed so the max-heap pops the cheapest edge. Costs are finite and non-negative
    /// ([`Quadric::error`]), so the order is total.
    fn cmp(&self, other: &Candidate) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Candidate) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Where an edge would collapse to, its cost, and how far it would move the surface.
struct Placement {
    at: Vec3,
    cost: f64,
    deviation: f64,
    /// The end that stays, and the end that goes.
    keep: u32,
    drop: u32,
}

/// Collapse edges cheapest first until `target` triangles or nothing is collapsible.
/// Returns the surface deviation in millimetres, or `None` if abandoned (half-simplified).
pub(crate) fn run(surface: &mut Surface, plan: &Simplify, target: usize, give_up: Abandon<'_>) -> Option<f64> {
    let mut stamps = vec![0u32; surface.positions.len()];
    let mut queue: BinaryHeap<Candidate> = BinaryHeap::new();
    for v in 0..surface.positions.len() as u32 {
        for n in surface.neighbours(v) {
            if n > v {
                if let Some(placement) = place(surface, plan, v, n) {
                    queue.push(Candidate {
                        cost: placement.cost,
                        ends: [v, n],
                        stamps: [stamps[v as usize], stamps[n as usize]],
                    });
                }
            }
        }
    }
    let mut moved: f64 = 0.0;
    // Check cancellation every thousandth collapse, since the flag is a shared atomic.
    let mut since_asked = 0u32;
    while surface.live > target {
        since_asked += 1;
        if since_asked >= 1024 {
            since_asked = 0;
            if give_up() {
                return None;
            }
        }
        let Some(candidate) = queue.pop() else { break };
        let [a, b] = candidate.ends;
        if candidate.stamps != [stamps[a as usize], stamps[b as usize]] {
            continue;
        }
        let Some(placement) = place(surface, plan, a, b) else { continue };
        if !safe(surface, &placement) {
            continue;
        }
        moved = moved.max(placement.deviation);
        let kept = apply(surface, &placement);
        stamps[kept as usize] += 1;
        for n in surface.neighbours(kept) {
            stamps[n as usize] += 1;
        }
        for n in surface.neighbours(kept) {
            if let Some(placement) = place(surface, plan, kept, n) {
                queue.push(Candidate {
                    cost: placement.cost,
                    ends: [kept, n],
                    stamps: [stamps[kept as usize], stamps[n as usize]],
                });
            }
        }
    }
    Some(moved)
}

/// Where edge `a`-`b` would go, or `None` if it may not collapse.
fn place(surface: &Surface, plan: &Simplify, a: u32, b: u32) -> Option<Placement> {
    let (locked_a, locked_b) = (surface.locked[a as usize], surface.locked[b as usize]);
    if locked_a && locked_b {
        return None;
    }
    let (pa, pb) = (surface.positions[a as usize], surface.positions[b as usize]);
    let mut quadric = surface.quadrics[a as usize];
    quadric.add(&surface.quadrics[b as usize]);
    // A locked end keeps its feature in place and takes the collapse; only two free ends choose a position.
    let at = match (locked_a, locked_b) {
        (true, _) => pa,
        (_, true) => pb,
        _ => best_position(&quadric, pa, pb),
    };
    let cost = quadric.error(at);
    let deviation = deviation_of(cost, surface.weight[a as usize] + surface.weight[b as usize]);
    if plan.limit_deviation && deviation > plan.max_deviation {
        return None;
    }
    let (keep, drop) = if locked_b { (b, a) } else { (a, b) };
    Some(Placement { at, cost, deviation, keep, drop })
}

/// A quadric error as a millimetre distance: the area-weighted mean distance from the original
/// planes, independent of model scale.
///
/// Deliberately not a bound: a worst-case bound assumes all collapses move the same way, and on a
/// 20 mm sphere reported 13.7 mm against an actual 2.5 mm.
pub(crate) fn deviation_of(error: f64, weight: f64) -> f64 {
    if weight <= 0.0 {
        return 0.0;
    }
    (error / weight).sqrt()
}

/// The optimal point for the combined quadric, if it stays near the edge; otherwise the best of
/// the two ends and the middle, since a far-off optimum is a spike through the model.
fn best_position(quadric: &Quadric, a: Vec3, b: Vec3) -> Vec3 {
    let middle = a.lerp(b, 0.5);
    if let Some(at) = quadric.optimal(quadric.scale()) {
        if (at - middle).length() <= (b - a).length() {
            return at;
        }
    }
    [a, b, middle]
        .into_iter()
        .min_by(|one, two| quadric.error(*one).partial_cmp(&quadric.error(*two)).unwrap_or(Ordering::Equal))
        .unwrap_or(middle)
}

/// Whether the collapse keeps the surface manifold and uninverted.
///
/// Topologically, the two ends' rings must share only the edge's two triangles (the link
/// condition), or the collapse makes a three-face edge. Geometrically, no triangle may flip by
/// more than ninety degrees, which would turn a patch inside out.
fn safe(surface: &Surface, placement: &Placement) -> bool {
    let (keep, drop) = (placement.keep, placement.drop);
    let along = surface.along(keep, drop);
    match along.len() {
        // An interior edge of a closed surface, the ordinary case.
        2 => {}
        // A hole's rim, reached only when boundaries are not kept.
        1 => {}
        _ => return false,
    }
    let opposite: Vec<u32> = along
        .iter()
        .filter_map(|&t| surface.tris[t as usize].iter().copied().find(|&v| v != keep && v != drop))
        .collect();
    let shared: Vec<u32> =
        surface.neighbours(keep).into_iter().filter(|v| surface.neighbours(drop).contains(v)).collect();
    if shared.len() != opposite.len() || !shared.iter().all(|v| opposite.contains(v)) {
        return false;
    }
    for &t in surface.incident[keep as usize].iter().chain(surface.incident[drop as usize].iter()) {
        if !surface.alive[t as usize] || along.contains(&t) {
            continue;
        }
        let before = surface.normal_of(surface.tris[t as usize]);
        let moved = surface.tris[t as usize].map(|v| match v == keep || v == drop {
            true => placement.at,
            false => surface.positions[v as usize],
        });
        let after = (moved[1] - moved[0]).cross(moved[2] - moved[0]);
        if after.length() <= 0.0 || before.dot(after.normalized()) <= 0.0 {
            return false;
        }
    }
    true
}

/// Make the collapse: remove the edge's triangles, move the dropped vertex's others to the kept
/// one, and merge its quadric.
fn apply(surface: &mut Surface, placement: &Placement) -> u32 {
    let (keep, drop) = (placement.keep, placement.drop);
    for t in surface.along(keep, drop) {
        surface.alive[t as usize] = false;
        surface.live -= 1;
    }
    let moving: Vec<u32> =
        surface.incident[drop as usize].iter().copied().filter(|&t| surface.alive[t as usize]).collect();
    for t in moving {
        for v in surface.tris[t as usize].iter_mut() {
            if *v == drop {
                *v = keep;
            }
        }
        surface.incident[keep as usize].push(t);
    }
    surface.incident[drop as usize].clear();
    surface.positions[keep as usize] = placement.at;
    let dropped = surface.quadrics[drop as usize];
    surface.quadrics[keep as usize].add(&dropped);
    surface.weight[keep as usize] += surface.weight[drop as usize];
    surface.tidy(keep);
    keep
}

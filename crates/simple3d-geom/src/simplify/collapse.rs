//! Choosing the next edge to collapse, checking it is safe, and doing it.

use super::*;
use crate::vec3::Vec3;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// One edge waiting its turn, with what it cost when it was put in the queue.
///
/// The cost goes stale: collapsing an edge changes the quadric of the vertex it
/// leaves behind, and so the price of every edge at that vertex. Rather than
/// find those entries and correct them -- which means an index from vertices to
/// queue positions, kept right through every collapse -- each entry remembers
/// how many times its two ends had been touched when it was made. An entry
/// whose ends have moved on since is thrown away when it comes up, and the edge
/// is already back in the queue at its new price.
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
    /// Reversed, so the standard max-heap hands back the *cheapest* edge: the
    /// one whose collapse moves the surface least is the one to make first.
    /// Costs are finite and never negative -- see [`Quadric::error`] -- so the
    /// partial order is a total one here.
    fn cmp(&self, other: &Candidate) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Candidate) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Where an edge would collapse to, what it would cost, and how far it would
/// leave the surface from where it started.
struct Placement {
    at: Vec3,
    cost: f64,
    deviation: f64,
    /// The end that stays, and the end that goes.
    keep: u32,
    drop: u32,
}

/// Collapse edges, cheapest first, until the mesh is down to `target`
/// triangles or nothing is left that may be collapsed.
///
/// Gives back how far the surface was moved, in millimetres, or `None` when the
/// run was abandoned part-way -- in which case the surface is half-simplified
/// and is not an answer.
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
    // Every thousandth collapse, rather than every one: the flag is an atomic
    // read shared with the thread that sets it, and a simplification is
    // hundreds of thousands of collapses.
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

/// Where the edge `a`-`b` would go, or `None` when it may not go anywhere.
fn place(surface: &Surface, plan: &Simplify, a: u32, b: u32) -> Option<Placement> {
    let (locked_a, locked_b) = (surface.locked[a as usize], surface.locked[b as usize]);
    if locked_a && locked_b {
        return None;
    }
    let (pa, pb) = (surface.positions[a as usize], surface.positions[b as usize]);
    let mut quadric = surface.quadrics[a as usize];
    quadric.add(&surface.quadrics[b as usize]);
    // A fixed end takes the collapse to itself: the feature it sits on stays
    // exactly where it was, and the free vertex beside it is the one that goes.
    // Only when both ends are free is there a position to choose.
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

/// A quadric's error read as a distance in millimetres: how far the surface
/// stands, on average over its own area, from the planes it was made of.
///
/// The error itself cannot be shown to anybody. It is a sum over every original
/// triangle a vertex has swallowed, weighted by that triangle's area, so it
/// grows with the size of the shape and with how much of it one vertex now
/// stands for -- the same collapse costs a hundred times more on a part
/// measured in centimetres than on one measured in millimetres. Dividing by the
/// area first takes both of those out and leaves a length.
///
/// It is not a bound, and it is deliberately not one. The bound is easy --
/// carry, at each vertex, the furthest any point it swallowed could now be
/// from where it started -- but it assumes every collapse moved the surface the
/// same way, and collapses on a curved surface move it in opposite directions
/// about equally often. On a 20 mm sphere taken down to a tenth of its
/// triangles that bound reported 13.7 mm, against a surface that had actually
/// moved 2.5 mm: a guarantee big enough to refuse everything is worth less than
/// a measure that says what happened.
pub(crate) fn deviation_of(error: f64, weight: f64) -> f64 {
    if weight <= 0.0 {
        return 0.0;
    }
    (error / weight).sqrt()
}

/// The point the pair of quadrics is happiest with.
///
/// The solved optimum is used when there is one and it stays near the edge it
/// replaces. It can be far away and still be optimal -- two nearly parallel
/// planes meet a long way off -- and a vertex that lands there is a spike
/// through the model, so the fallback is the best of the two ends and the
/// middle, which are the three points that cannot leave the surface.
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

/// Whether the collapse can be made without tearing the surface or turning a
/// triangle inside out.
///
/// Two separate questions, and both have to be asked every time.
///
/// The first is topological. Collapsing an edge merges the ring of triangles
/// around one end into the ring around the other, and that is only a surface
/// again when the two rings share exactly the two triangles along the edge
/// itself -- the *link condition*. Where they share a third vertex somewhere
/// else, the collapse folds a tube into a sheet and leaves an edge with three
/// faces on it, which no slicer will take and no later collapse can repair.
///
/// The second is geometric. A collapse that passes the link condition can still
/// drag a vertex through its own neighbours, leaving triangles that face
/// backwards -- the surface turned inside out in a small patch, which reads as
/// a black hole in the shading and exports as a solid with its inside out. Any
/// triangle that would come out facing more than ninety degrees from where it
/// faced before refuses the collapse.
fn safe(surface: &Surface, placement: &Placement) -> bool {
    let (keep, drop) = (placement.keep, placement.drop);
    let along = surface.along(keep, drop);
    match along.len() {
        // An interior edge of a closed surface, which is the ordinary case.
        2 => {}
        // The rim of a hole, reached only when the settings do not ask for
        // boundaries to be kept.
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

/// Make the collapse: strike out the triangles along the edge, hand the rest of
/// the dropped vertex's triangles to the one that stays, and give it the
/// dropped vertex's quadric so it remembers the planes it now stands for.
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

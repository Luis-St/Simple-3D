//! Joining edges into a path, and following a round edge's run.

use super::*;
use std::collections::{HashMap, VecDeque};

fn same(a: Vec3, b: Vec3) -> bool {
    (a - b).length() <= SAME_POINT
}

/// The edges in connection order (see [`Path::from_edges`]).
pub(super) fn chain(edges: &[(Vec3, Vec3)]) -> Path {
    // A zero-length edge carries no direction, and one picked twice is one edge.
    let mut free: Vec<(Vec3, Vec3)> = Vec::new();
    for &(a, b) in edges {
        let known = free.iter().any(|&(c, d)| (same(a, c) && same(b, d)) || (same(a, d) && same(b, c)));
        if !same(a, b) && !known {
            free.push((a, b));
        }
    }
    if free.is_empty() {
        return Path::default();
    }
    let (a, b) = free.remove(0);
    let mut points: VecDeque<Vec3> = VecDeque::from([a, b]);
    while !free.is_empty() {
        let back = *points.back().expect("a chain has ends");
        let front = *points.front().expect("a chain has ends");
        // A touching edge first, at the back and then the front, so a run picked in order stays in order.
        let touching = free.iter().position(|&(a, b)| same(a, back) || same(b, back));
        if let Some(at) = touching {
            let (a, b) = free.remove(at);
            points.push_back(if same(a, back) { b } else { a });
            continue;
        }
        let touching = free.iter().position(|&(a, b)| same(a, front) || same(b, front));
        if let Some(at) = touching {
            let (a, b) = free.remove(at);
            points.push_front(if same(a, front) { b } else { a });
            continue;
        }
        // Nothing touches: bridge to the nearest end of any free edge, from whichever chain end is nearer.
        let mut best = (f64::INFINITY, 0, false, false);
        for (index, &(a, b)) in free.iter().enumerate() {
            for (at_back, end) in [(true, back), (false, front)] {
                for (flip, near) in [(false, a), (true, b)] {
                    let gap = (near - end).length();
                    if gap < best.0 {
                        best = (gap, index, at_back, flip);
                    }
                }
            }
        }
        let (_, index, at_back, flip) = best;
        let (a, b) = free.remove(index);
        let (near, far) = if flip { (b, a) } else { (a, b) };
        if at_back {
            points.push_back(near);
            points.push_back(far);
        } else {
            points.push_front(near);
            points.push_front(far);
        }
    }
    let mut points: Vec<Vec3> = points.into();
    let closed = points.len() > 3 && same(points[0], *points.last().expect("a chain has ends"));
    if closed {
        points.pop();
    }
    Path { points, closed }
}

/// The largest turn, in degrees, a run of edges takes at a corner and still counts as one edge.
const SMOOTH_TURN: f64 = 40.0;

/// A cap on a run's length, so a fine sphere cannot walk every edge it has.
const MOST_EDGES: usize = 4096;

/// The run of `edges` that `start` belongs to: continued at each end through the edge turning least,
/// while that turn is gentle. A cylinder's rim comes back whole, while a box edge stops at its
/// corners.
pub fn smooth_run(edges: &[(Vec3, Vec3)], start: (Vec3, Vec3)) -> Vec<(Vec3, Vec3)> {
    let key = |p: Vec3| ((p.x * 1e3).round() as i64, (p.y * 1e3).round() as i64, (p.z * 1e3).round() as i64);
    let mut at: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (index, &(a, b)) in edges.iter().enumerate() {
        at.entry(key(a)).or_default().push(index);
        at.entry(key(b)).or_default().push(index);
    }
    let mut run = vec![start];
    let mut used: Vec<usize> = edges
        .iter()
        .enumerate()
        .filter(|(_, &(a, b))| (same(a, start.0) && same(b, start.1)) || (same(a, start.1) && same(b, start.0)))
        .map(|(index, _)| index)
        .collect();
    // Out from each end in turn: from `b` onwards, then from `a` backwards.
    for (from, to) in [(start.0, start.1), (start.1, start.0)] {
        let (mut from, mut to) = (from, to);
        while run.len() < MOST_EDGES {
            let heading = (to - from).normalized();
            let next = at.get(&key(to)).into_iter().flatten().copied().filter(|index| !used.contains(index)).min_by(
                |&i, &j| {
                    let turn = |index: usize| {
                        let (a, b) = edges[index];
                        let far = if same(a, to) { b } else { a };
                        -(far - to).normalized().dot(heading)
                    };
                    turn(i).partial_cmp(&turn(j)).unwrap_or(std::cmp::Ordering::Equal)
                },
            );
            let Some(index) = next else { break };
            let (a, b) = edges[index];
            let far = if same(a, to) { b } else { a };
            if (far - to).normalized().dot(heading) < SMOOTH_TURN.to_radians().cos() {
                break;
            }
            used.push(index);
            run.push((to, far));
            // Round the whole loop: back at the start, so the other direction has nothing left to add.
            if same(far, start.0) || same(far, start.1) {
                return run;
            }
            (from, to) = (to, far);
        }
    }
    run
}

//! Moving an outline's edges in or out by a distance, which a draft needs at every height.

use crate::{cross, dot, sub};
use simple3d_geom::push_pull::Outline;

/// `outline` with every edge moved `inward` into the solid (outward when negative), each corner on
/// the mitre of its two edges so corners stay corners and the loops keep their point count. `None`
/// when the move would turn an edge round, fold a loop over itself or collapse a spike: the draft is
/// too steep for that outline.
pub(crate) fn offset(outline: &Outline, inward: f64) -> Option<Outline> {
    if inward == 0.0 {
        return Some(outline.clone());
    }
    let outer = offset_loop(&outline.outer, inward)?;
    let holes = outline.holes.iter().map(|hole| offset_loop(hole, inward)).collect::<Option<Vec<_>>>()?;
    Some(Outline { outer, holes })
}

fn left_normal(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let d = sub(b, a);
    let len = crate::length(d).max(1e-12);
    [-d[1] / len, d[0] / len]
}

fn offset_loop(points: &[[f64; 2]], t: f64) -> Option<Vec<[f64; 2]>> {
    let n = points.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (prev, p, next) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let (n1, n2) = (left_normal(prev, p), left_normal(p, next));
        let bend = 1.0 + dot(n1, n2);
        // A corner folded nearly back on itself would send its mitre off to infinity.
        if bend < 0.05 {
            return None;
        }
        out.push([p[0] + (n1[0] + n2[0]) * t / bend, p[1] + (n1[1] + n2[1]) * t / bend]);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        if dot(sub(points[j], points[i]), sub(out[j], out[i])) <= 0.0 {
            return None;
        }
    }
    if crossing(&out) {
        return None;
    }
    Some(out)
}

/// Whether any two edges of the closed polygon cross, apart from neighbours meeting at a corner.
fn crossing(points: &[[f64; 2]]) -> bool {
    let n = points.len();
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (c, d) = (points[j], points[(j + 1) % n]);
            let side = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| cross(sub(q, p), sub(r, p));
            if side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0 {
                return true;
            }
        }
    }
    false
}

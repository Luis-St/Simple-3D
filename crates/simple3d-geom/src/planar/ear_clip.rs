//! Ear clipping: a simple polygon to triangles.

use super::*;
/// Ear-clip a counter-clockwise loop into `out`, never losing a boundary vertex: zero-area ears
/// (collinear T-junction vertices) are not clipped, or the T-junction would reopen. The exception is
/// a bridge seam, where the vertex appears twice and the other copy keeps it.
pub(crate) fn ear_clip(ids: &[u32], points: &[Point], out: &mut Vec<[u32; 3]>) -> Option<()> {
    let mut remaining: Vec<usize> = (0..ids.len()).collect();
    let mut guard = ids.len() * ids.len() + 16;
    while remaining.len() > 3 {
        guard = guard.checked_sub(1)?;
        let n = remaining.len();
        let mut clipped = None;
        let mut seam = None;
        for i in 0..n {
            let (ia, ib, ic) = (remaining[(i + n - 1) % n], remaining[i], remaining[(i + 1) % n]);
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            let turn = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
            if turn.abs() <= 1e-12 {
                if seam.is_none() && duplicated(ids, &remaining, ib) {
                    seam = Some(i);
                }
                continue;
            }
            if turn < 0.0 {
                continue; // reflex
            }
            if remaining.iter().any(|&j| j != ia && j != ib && j != ic && strictly_inside(points[j], a, b, c)) {
                continue;
            }
            // A loop vertex on the ear's new diagonal blocks it like one inside: two off-centre boxes leave an
            // inner corner in line with two outer ones, and clipping across it laid a flipped triangle over the
            // notch. Only a bridge-seam copy of `a` or `c` may lie on it.
            if remaining.iter().any(|&j| {
                let p = points[j];
                j != ia && j != ib && j != ic && p != a && p != c && on_open_segment(p, c, a)
            }) {
                continue;
            }
            clipped = Some((i, [ids[ia], ids[ib], ids[ic]]));
            break;
        }
        match clipped {
            Some((i, tri)) => {
                out.push(tri);
                remaining.remove(i);
            }
            // No real ear: unpicking a bridge seam may expose one; otherwise the caller keeps the original.
            None => {
                remaining.remove(seam?);
            }
        }
    }
    if remaining.len() == 3 {
        let (a, b, c) = (points[remaining[0]], points[remaining[1]], points[remaining[2]]);
        if ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs() <= 1e-12 {
            // Three collinear leftovers: emitting makes a sliver, dropping loses a boundary vertex.
            return None;
        }
        out.push([ids[remaining[0]], ids[remaining[1]], ids[remaining[2]]]);
    }
    Some(())
}

//! Ear clipping: a simple polygon to triangles.

use super::*;
/// Ear-clip a counter-clockwise loop, appending triangles to `out`.
///
/// Two rules beyond the textbook version, both about *not losing a vertex*.
///
/// The whole point of this pass is that the region's boundary comes out
/// unchanged, and a vertex that no emitted triangle mentions has silently left
/// the boundary -- reopening exactly the T-junction
/// [`crate::repair::split_t_junctions`] closed. So a zero-area ear (three
/// collinear vertices, which is what a healed boundary is full of) is never
/// clipped: its apex stays in the loop and gets used as a neighbour of some
/// other ear instead.
///
/// The exception is the seam a bridged hole leaves, where one vertex appears
/// twice in the loop. There a zero-area ear is exactly what should be removed,
/// and doing so loses nothing because the other copy still carries the vertex.
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
            clipped = Some((i, [ids[ia], ids[ib], ids[ic]]));
            break;
        }
        match clipped {
            Some((i, tri)) => {
                out.push(tri);
                remaining.remove(i);
            }
            // No real ear left. Unpicking a bridge seam may expose one; if there
            // is no seam either, this loop is beyond us and the caller keeps the
            // region's original triangles.
            None => {
                remaining.remove(seam?);
            }
        }
    }
    if remaining.len() == 3 {
        let (a, b, c) = (points[remaining[0]], points[remaining[1]], points[remaining[2]]);
        if ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs() <= 1e-12 {
            // Three collinear vertices left over: emitting them would be a
            // sliver, dropping them would lose a boundary vertex.
            return None;
        }
        out.push([ids[remaining[0]], ids[remaining[1]], ids[remaining[2]]]);
    }
    Some(())
}

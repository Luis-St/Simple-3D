//! Cutting one triangle by the section plane, and the edge each cut leaves
//! behind for the cap to close.

use super::*;
use crate::vec3::Vec3;

/// The part of `world` on the kept side of `plane`.
///
/// The usual answer is "all of it" or "none of it" -- a plane crosses a band of
/// triangles and misses every other one -- so both of those are decided on
/// three comparisons before anything is worked out.
///
/// A windowed plane takes a box out instead, which [`window::clip_box`] cuts;
/// that leaves no single cut edge, and its caps are found face by face (see
/// [`faces`]).
pub fn clip_triangle(plane: &Plane, world: [Vec3; 3]) -> Clipped {
    if plane.window.is_some() {
        return super::window::clip_box(&plane.walls(), world);
    }
    let d = [plane.depth(world[0]), plane.depth(world[1]), plane.depth(world[2])];
    // Wholly kept, which includes a triangle lying *in* the plane: it is the
    // surface the cut runs along, not something the cut removes.
    if d.iter().all(|&at| at <= 0.0) {
        return Clipped::whole(world);
    }
    if d.iter().all(|&at| at >= 0.0) {
        return Clipped::nothing();
    }

    // Sutherland-Hodgman against the one plane, in the triangle's own winding
    // so the polygon that comes out keeps the surface's orientation.
    let mut poly = [Vec3::ZERO; 4];
    let mut corners = 0;
    // Where the cut meets this triangle: the crossings of its edges, plus any
    // corner sitting exactly on the plane -- a plane through a box's own
    // vertices is the common case, not the exotic one, and a crossing counted
    // only where a *strict* sign change happens misses those.
    let mut hits: [Vec3; 3] = [Vec3::ZERO; 3];
    let mut hit_count = 0;
    for i in 0..3 {
        let j = (i + 1) % 3;
        if d[i] <= 0.0 {
            poly[corners] = world[i];
            corners += 1;
        }
        if d[i] == 0.0 && hit_count < 3 {
            hits[hit_count] = world[i];
            hit_count += 1;
        }
        if (d[i] < 0.0 && d[j] > 0.0) || (d[i] > 0.0 && d[j] < 0.0) {
            let t = d[i] / (d[i] - d[j]);
            let at = world[i] + (world[j] - world[i]) * t;
            poly[corners] = at;
            corners += 1;
            if hit_count < 3 {
                hits[hit_count] = at;
                hit_count += 1;
            }
        }
    }

    let mut out = Clipped::nothing();
    if corners >= 3 {
        out.triangles[0] = [poly[0], poly[1], poly[2]];
        out.count = 1;
        if corners == 4 {
            out.triangles[1] = [poly[0], poly[2], poly[3]];
            out.count = 2;
        }
        out.cut = cap_edge(world, &hits[..hit_count], plane);
    }
    out
}

/// The cut edge of one clipped triangle, wound the way the cap wants it.
///
/// The direction is not a guess. The cap closes the kept solid, so along their
/// shared edge it runs *against* the face the edge came off -- that is what
/// makes a closed surface closed. Walking the clipped face in its own winding,
/// the cut edge runs along `n x m` (the face's outward normal crossed with the
/// plane's), so the cap's runs along `m x n`. Every cap edge taken that way
/// chains into loops that are counter-clockwise about the plane normal for an
/// outer boundary and clockwise for a hole, which is exactly what the
/// triangulator reads them as.
pub(crate) fn cap_edge(world: [Vec3; 3], hits: &[Vec3], plane: &Plane) -> Option<[Vec3; 2]> {
    if hits.len() < 2 {
        return None;
    }
    let (from, to) = (hits[0], hits[hits.len() - 1]);
    if (to - from).length() < 1e-12 {
        return None;
    }
    let normal = (world[1] - world[0]).cross(world[2] - world[0]);
    let forward = plane.normal.cross(normal);
    // A face lying in the plane has no direction to take -- and no cut edge of
    // its own either: its neighbours draw the boundary around it.
    if forward.length() < 1e-12 {
        return None;
    }
    match (to - from).dot(forward) >= 0.0 {
        true => Some([from, to]),
        false => Some([to, from]),
    }
}

/// The part of the segment `a`-`b` on the kept side, or `None` when the whole
/// of it is cut away. What every *line* of the picture goes through: a feature
/// edge, a selection outline, the mark a principal plane leaves on a surface.
pub fn clip_segment(plane: &Plane, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
    let (da, db) = (plane.depth(a), plane.depth(b));
    if da <= 0.0 && db <= 0.0 {
        return Some((a, b));
    }
    if da > 0.0 && db > 0.0 {
        return None;
    }
    let at = a + (b - a) * (da / (da - db));
    match da <= 0.0 {
        true => Some((a, at)),
        false => Some((at, b)),
    }
}

/// What is left of a segment once the cuts have had it: nothing, all of it, a
/// piece off one end -- or, past a windowed plane, the two ends with the box
/// taken out of the middle, once per windowed plane.
#[derive(Clone, Copy, Debug)]
pub struct Segments {
    pieces: [(Vec3, Vec3); MAX_CUTS + 1],
    count: usize,
}

impl Default for Segments {
    fn default() -> Segments {
        Segments { pieces: [(Vec3::ZERO, Vec3::ZERO); MAX_CUTS + 1], count: 0 }
    }
}

impl Segments {
    /// A segment nothing cut.
    pub fn whole(a: Vec3, b: Vec3) -> Segments {
        let mut out = Segments::default();
        out.push(a, b);
        out
    }

    fn push(&mut self, a: Vec3, b: Vec3) {
        if (b - a).length() > 1e-12 && self.count < self.pieces.len() {
            self.pieces[self.count] = (a, b);
            self.count += 1;
        }
    }
}

impl std::ops::Deref for Segments {
    type Target = [(Vec3, Vec3)];
    fn deref(&self) -> &[(Vec3, Vec3)] {
        &self.pieces[..self.count]
    }
}

/// The parts of `a`-`b` that stay, whatever shape the cut is. The stretch the
/// cut takes out is where the segment is past every wall at once, which is one
/// interval along it, found by narrowing it wall by wall.
pub fn kept_segments(plane: &Plane, a: Vec3, b: Vec3) -> Segments {
    let mut out = Segments::default();
    if plane.window.is_none() {
        if let Some((from, to)) = clip_segment(plane, a, b) {
            out.push(from, to);
        }
        return out;
    }
    let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
    for wall in plane.walls().iter() {
        let (da, db) = (wall.depth(a), wall.depth(b));
        // Inside the box is where the depth is positive.
        if da <= 0.0 && db <= 0.0 {
            out.push(a, b);
            return out;
        }
        if da > 0.0 && db > 0.0 {
            continue;
        }
        let t = da / (da - db);
        match da <= 0.0 {
            true => enter = enter.max(t),
            false => leave = leave.min(t),
        }
    }
    if enter >= leave {
        out.push(a, b);
        return out;
    }
    let at = |t: f64| a + (b - a) * t;
    out.push(a, at(enter));
    out.push(at(leave), b);
    out
}

/// The parts of `a`-`b` that every one of `cuts` keeps: each cut in turn, on
/// what the ones before it left.
pub fn kept_by_all(cuts: &[Plane], a: Vec3, b: Vec3) -> Segments {
    let mut kept = Segments::whole(a, b);
    for cut in cuts {
        let mut next = Segments::default();
        for &(from, to) in kept.iter() {
            for &(from, to) in kept_segments(cut, from, to).iter() {
                next.push(from, to);
            }
        }
        kept = next;
        if kept.is_empty() {
            break;
        }
    }
    kept
}

/// What is left of a triangle once every one of `cuts` has had it.
pub fn clip_by_all(cuts: &[Plane], world: [Vec3; 3]) -> Clipped {
    match cuts {
        [] => Clipped::untouched(world),
        [only] => clip_triangle(only, world),
        _ => {
            let mut pieces = vec![world];
            for cut in cuts {
                pieces = pieces.iter().flat_map(|&piece| clip_triangle(cut, piece).triangles().to_vec()).collect();
                if pieces.is_empty() {
                    break;
                }
            }
            Clipped::pieces(pieces)
        }
    }
}

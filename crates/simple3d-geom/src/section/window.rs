//! A section plane cut down to a rectangle: only the material straight behind
//! the rectangle goes, which takes a box out of the model rather than half of
//! it.

use super::*;
use crate::vec3::Vec3;

/// The rectangle a windowed [`Plane`] cuts within: its middle, which lies in
/// the plane, the two directions it spans, and its half-width along each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub centre: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    pub half: [f64; 2],
}

/// The part of a convex polygon on the side of `wall` its depth is at most
/// zero on, when `kept` -- or the rest of it otherwise.
pub fn clip_polygon(polygon: &[Vec3], wall: &Plane, kept: bool) -> Vec<Vec3> {
    let side = |p: Vec3| match kept {
        true => wall.depth(p),
        false => -wall.depth(p),
    };
    let mut out = Vec::with_capacity(polygon.len() + 1);
    for (index, &from) in polygon.iter().enumerate() {
        let to = polygon[(index + 1) % polygon.len()];
        let (df, dt) = (side(from), side(to));
        if df <= 0.0 {
            out.push(from);
        }
        if (df < 0.0 && dt > 0.0) || (df > 0.0 && dt < 0.0) {
            out.push(from + (to - from) * (df / (df - dt)));
        }
    }
    out
}

/// A convex polygon as a fan of triangles, in its own winding.
fn fan(polygon: &[Vec3], out: &mut Vec<[Vec3; 3]>) {
    for index in 1..polygon.len().saturating_sub(1) {
        out.push([polygon[0], polygon[index], polygon[index + 1]]);
    }
}

/// What is left of a triangle with the box behind a window taken out of it.
///
/// The part outside the box is split into pieces that do not overlap: what is
/// in front of the first wall, then what is behind that one but in front of the
/// second, and so on -- each a convex polygon, so each a fan.
pub(super) fn clip_box(walls: &[Plane], world: [Vec3; 3]) -> Clipped {
    // Wholly in front of any one wall is wholly kept, which is nearly every
    // triangle of the model.
    if walls.iter().any(|wall| world.iter().all(|&p| wall.depth(p) <= 0.0)) {
        return Clipped::whole(world);
    }
    if world.iter().all(|&p| walls.iter().all(|wall| wall.depth(p) >= 0.0)) {
        return Clipped::nothing();
    }
    let mut inside: Vec<Vec3> = world.to_vec();
    let mut pieces = Vec::new();
    for wall in walls {
        fan(&clip_polygon(&inside, wall, true), &mut pieces);
        inside = clip_polygon(&inside, wall, false);
        if inside.len() < 3 {
            break;
        }
    }
    match pieces.is_empty() {
        true => Clipped::nothing(),
        false => Clipped::pieces(pieces),
    }
}

/// One face of what the cut opens up: a plane the model's inside shows through,
/// and the part of it that is really open.
#[derive(Clone, Debug)]
pub struct Face {
    /// The face, facing into what was cut away, with no window.
    pub plane: Plane,
    /// The other walls: the face is open only where every one of them has a
    /// depth of at least zero, which is the side of the face the box is.
    /// Empty for a plane that cuts everywhere.
    pub bounds: Vec<Plane>,
}

/// The faces a cut opens: the plane alone, or the rectangle and the four sides
/// of the box behind it. Each is capped as a plane of its own and then cut down
/// to its face.
pub fn faces(plane: &Plane) -> Vec<Face> {
    let walls = plane.walls();
    (0..walls.len())
        .map(|index| Face {
            plane: walls[index],
            bounds: walls.iter().enumerate().filter(|&(other, _)| other != index).map(|(_, wall)| *wall).collect(),
        })
        .collect()
}

/// A convex polygon cut down to where every one of `bounds` has a depth of at
/// least zero.
pub fn within(polygon: &[Vec3], bounds: &[Plane]) -> Vec<Vec3> {
    let mut out = polygon.to_vec();
    for bound in bounds {
        if out.len() < 3 {
            return Vec::new();
        }
        out = clip_polygon(&out, bound, false);
    }
    out
}

/// The part of `a`-`b` where every one of `bounds` has a depth of at least
/// zero, or `None` when there is none.
pub fn segment_within(a: Vec3, b: Vec3, bounds: &[Plane]) -> Option<(Vec3, Vec3)> {
    let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
    for bound in bounds {
        let (da, db) = (bound.depth(a), bound.depth(b));
        if da < 0.0 && db < 0.0 {
            return None;
        }
        if da >= 0.0 && db >= 0.0 {
            continue;
        }
        let t = da / (da - db);
        match da < 0.0 {
            true => enter = enter.max(t),
            false => leave = leave.min(t),
        }
    }
    (enter < leave).then(|| (a + (b - a) * enter, a + (b - a) * leave))
}

/// Whether the cut runs through this triangle: the plane crosses it, and with
/// a window, crosses it inside the window.
pub fn triangle_touches(plane: &Plane, world: [Vec3; 3]) -> bool {
    let d = world.map(|p| plane.depth(p));
    if d.iter().all(|&at| at > 0.0) || d.iter().all(|&at| at < 0.0) {
        return false;
    }
    let Some(window) = plane.window else { return true };
    // The piece of the plane the triangle crosses it in, tested against the
    // rectangle.
    let mut hits = Vec::with_capacity(3);
    for i in 0..3 {
        let j = (i + 1) % 3;
        if d[i] == 0.0 {
            hits.push(world[i]);
        }
        if (d[i] < 0.0 && d[j] > 0.0) || (d[i] > 0.0 && d[j] < 0.0) {
            hits.push(world[i] + (world[j] - world[i]) * (d[i] / (d[i] - d[j])));
        }
    }
    let walls = Plane { window: Some(window), ..*plane }.walls();
    let bounds = &walls[1..];
    match hits.len() {
        0 => false,
        1 => bounds.iter().all(|wall| wall.depth(hits[0]) >= 0.0),
        _ => segment_within(hits[0], hits[hits.len() - 1], bounds).is_some(),
    }
}

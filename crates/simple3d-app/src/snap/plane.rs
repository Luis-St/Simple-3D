//! The marks a principal plane leaves on a body.

use simple3d_geom::{Mesh, Vec3};

/// The lines the principal planes leave on one body's surface: the marks the
/// renderer draws on the solid itself, where each plane through the origin cuts
/// it, as segments in world space.
///
/// A mark is a chain of short segments, one per triangle the plane crosses --
/// exactly what is drawn, and exactly what a nearest-point search wants, since
/// the nearest point on a chain is the nearest point on one of its links. So
/// nothing joins them up.
///
/// `axes` says which of the three switches are on; [`MARK_AXIS`] says which
/// plane each of them marks. It is the same rule the drawing uses, so a plane
/// whose mark is not on screen is not caught either.
pub fn plane_mark_lines(mesh: &Mesh, axes: [bool; 3]) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    for (switch, &shown) in axes.iter().enumerate() {
        if !shown {
            continue;
        }
        let axis = MARK_AXIS[switch];
        for tri in &mesh.indices {
            let world =
                [mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]];
            if let Some(segment) = plane_crossing(world, axis) {
                out.push(segment);
            }
        }
    }
    out
}

/// Which axis a principal plane's mark is presented as, indexed either way:
/// the plane perpendicular to `axis` is drawn as `MARK_AXIS[axis]`, and the
/// switch for `axis` governs the plane perpendicular to `MARK_AXIS[axis]`. The
/// table is its own inverse, which is what lets one constant answer both.
///
/// X and Y are exchanged. The mark left by the plane perpendicular to X is
/// drawn in Y's green, and the one perpendicular to Y in X's red -- the opposite
/// way round from the axis lines, which keep the usual X red / Y green. That was
/// asked for: on a shape standing on the origin the conventional pairing reads
/// as the wrong way round, and the mark on the surface is what is being read at
/// that moment.
///
/// The switches have to follow the same exchange (issue 75). A mark is named by
/// the colour it is drawn in -- that is all there is to go on when looking at
/// one -- so a red mark that goes out when the *Y* box is unticked is worse than
/// either pairing on its own.
pub const MARK_AXIS: [usize; 3] = [1, 0, 2];

/// Where the plane through the origin perpendicular to `axis` crosses one
/// triangle, as the segment it cuts. `None` when the triangle is wholly on one
/// side, which is nearly all of them, so this is the cheap case.
///
/// The renderer draws the marks and the measure tool catches them, and neither
/// may see a line the other does not, so both ask this.
pub fn plane_crossing(world: [Vec3; 3], axis: usize) -> Option<(Vec3, Vec3)> {
    let d = [component(world[0], axis), component(world[1], axis), component(world[2], axis)];
    if (d[0] > 0.0 && d[1] > 0.0 && d[2] > 0.0) || (d[0] < 0.0 && d[1] < 0.0 && d[2] < 0.0) {
        return None;
    }
    // A triangle lying *in* the plane has no crossing line of its own -- its
    // three edges are the mark, and its neighbours draw them.
    if d[0] == 0.0 && d[1] == 0.0 && d[2] == 0.0 {
        return None;
    }
    let mut hits: Vec<Vec3> = Vec::new();
    for i in 0..3 {
        let j = (i + 1) % 3;
        if d[i] == 0.0 {
            hits.push(world[i]);
        }
        if (d[i] < 0.0 && d[j] > 0.0) || (d[i] > 0.0 && d[j] < 0.0) {
            let t = d[i] / (d[i] - d[j]);
            hits.push(world[i] + (world[j] - world[i]) * t);
        }
    }
    (hits.len() >= 2).then(|| (hits[0], hits[1]))
}

pub(crate) fn component(v: Vec3, axis: usize) -> f64 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

pub(crate) fn set_component(v: &mut Vec3, axis: usize, value: f64) {
    match axis {
        0 => v.x = value,
        1 => v.y = value,
        _ => v.z = value,
    }
}

/// Where an infinite line meets a triangle. The same intersection the picker
/// uses, without its "ahead of the origin only" rule: an axis runs both ways
/// from the origin and crosses bodies on both sides of it.
pub(crate) fn line_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(h) * inv;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    Some(e2.dot(q) * inv)
}

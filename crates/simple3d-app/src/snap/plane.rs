//! The marks a principal plane leaves on a body.

use simple3d_geom::{Mesh, Vec3};

/// The principal-plane marks on one body's surface as world segments, one per crossed triangle,
/// exactly as drawn (a chain's nearest point is on one of its links). `axes` are the switches;
/// [`MARK_AXIS`] maps them to planes, as the drawing does.
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

/// Which axis a plane's mark is presented as; its own inverse. X and Y are swapped (the plane
/// perpendicular to X is drawn in Y's green), as requested, since the conventional pairing read
/// backwards on the surface. The switches follow the same swap (issue 75), since marks are named by
/// their colour.
pub const MARK_AXIS: [usize; 3] = [1, 0, 2];

/// Where the origin plane perpendicular to `axis` crosses one triangle, or `None` (the cheap common
/// case). Shared by the renderer and the measure tool so they never disagree.
pub fn plane_crossing(world: [Vec3; 3], axis: usize) -> Option<(Vec3, Vec3)> {
    let d = [component(world[0], axis), component(world[1], axis), component(world[2], axis)];
    if (d[0] > 0.0 && d[1] > 0.0 && d[2] > 0.0) || (d[0] < 0.0 && d[1] < 0.0 && d[2] < 0.0) {
        return None;
    }
    // A triangle lying in the plane has no crossing of its own; its neighbours draw its edges.
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

/// Where an infinite line meets a triangle: the picker's test without its "ahead only" rule, since
/// an axis runs both ways.
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

//! Revolving: the outlines turned about a line in their own plane.

use super::{caps, degenerate, loops};
use crate::{cross, dot, sub};
use simple3d_geom::push_pull::Outline;
use simple3d_geom::{Mesh, Vec3};

/// The outlines revolved `sweep` degrees about the line through `axis.0` and `axis.1`, turning
/// right-handed about that direction, with `segments` steps to a full turn. A partial sweep is closed
/// by the outline itself at both ends. Fails when the profile lies across the axis, which would
/// sweep it through itself.
pub fn revolve(outlines: &[Outline], axis: ([f64; 2], [f64; 2]), sweep: f64, segments: u32) -> Result<Mesh, String> {
    if outlines.is_empty() {
        return Err("No region is picked to revolve".into());
    }
    let (a, b) = axis;
    let length = crate::distance(a, b);
    if length < 1e-9 {
        return Err("The revolve axis has no length".into());
    }
    let d = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
    let n_left = [-d[1], d[0]];
    let sweep = sweep.clamp(0.0, 360.0);
    if sweep < 1e-6 {
        return Err("The revolve has no angle".into());
    }
    let reach = |p: [f64; 2]| cross(d, sub(p, a));
    let all: Vec<[f64; 2]> = loops(outlines).into_iter().flatten().copied().collect();
    let left = all.iter().any(|&p| reach(p) > 1e-6);
    let right = all.iter().any(|&p| reach(p) < -1e-6);
    if left && right {
        return Err("The profile lies across the revolve axis; pick regions on one side of it".into());
    }
    let full = (sweep - 360.0).abs() < 1e-9;
    let steps = ((segments.max(8) as f64 * sweep / 360.0).ceil() as usize).max(3);
    // Point `p` of the sketch turned `theta` about the axis.
    let turn = |p: [f64; 2], theta: f64| {
        let (along, r) = (dot(sub(p, a), d), reach(p));
        let (s, c) = theta.sin_cos();
        Vec3::new(a[0] + d[0] * along + n_left[0] * r * c, a[1] + d[1] * along + n_left[1] * r * c, r * s)
    };
    let angle = |j: usize| sweep.to_radians() * j as f64 / steps as f64;
    let mut mesh = Mesh::new();
    let push = |p: Vec3, q: Vec3, r: Vec3, mesh: &mut Mesh| {
        if !degenerate(p, q, r) {
            mesh.push_triangle(p, q, r);
        }
    };
    for points in loops(outlines) {
        let n = points.len();
        for i in 0..n {
            let (p, q) = (points[i], points[(i + 1) % n]);
            for j in 0..steps {
                let k = if full && j + 1 == steps { 0 } else { j + 1 };
                let (a0, a1) = (turn(p, angle(j)), turn(p, angle(k)));
                let (b0, b1) = (turn(q, angle(j)), turn(q, angle(k)));
                push(a0, b1, a1, &mut mesh);
                push(a0, b0, b1, &mut mesh);
            }
        }
    }
    if !full {
        let cap = caps(outlines, 0.0).ok_or("The profile cannot be filled to close the solid")?;
        let end = angle(steps);
        for [p, q, r] in cap {
            let flat = |v: Vec3| [v.x, v.y];
            push(p, r, q, &mut mesh);
            push(turn(flat(p), end), turn(flat(q), end), turn(flat(r), end), &mut mesh);
        }
    }
    // Built for a profile left of the axis; on its right everything comes out mirrored.
    if right {
        mesh.flip_winding();
    }
    Ok(mesh)
}

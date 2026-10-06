//! The solid that rounds or bevels one edge: a cross-section swept along it.

use super::mitre::{sweep, End};
use super::*;
use crate::mesh::Mesh;
use crate::push_pull::Outline;
use crate::vec3::Vec3;

/// An edge's treatment seen end on: the curve or chord it leaves, as offsets from a point of the edge.
#[derive(Clone, Debug)]
pub struct Profile {
    /// From the tangent point on the first face to the one on the second.
    pub curve: Vec<Vec3>,
}

/// Where along the faces the treatment of `edge` reaches, and the curve between, as offsets from the
/// edge. `None` for faces too close to flat or folded flat to treat.
pub fn edge_profile(edge: &FeatureEdge, treatment: Treatment) -> Option<Profile> {
    let opening = edge.opening();
    if !(0.05..std::f64::consts::PI - 0.05).contains(&opening) {
        return None;
    }
    let reach = treatment.reach(opening);
    // Normals turned towards the side being worked: out of a convex edge's solid, out of a concave
    // edge's air.
    let sign = if edge.convex { 1.0 } else { -1.0 };
    let [n1, n2] = edge.normals.map(|n| n * sign);
    let (t1, t2) = (edge.along[0] * reach, edge.along[1] * reach);
    let curve = match treatment {
        Treatment::Round { radius, segments } => {
            let centre = t1 - n1 * radius;
            let k = segments.max(1);
            (0..=k).map(|i| centre + slerp(n1, n2, i as f64 / k as f64) * radius).collect()
        }
        Treatment::Chamfer { .. } => vec![t1, t2],
    };
    Some(Profile { curve })
}

/// The solid that treats `edge`: the sliver between the faces and the profile, grown out past the
/// faces so no face of it lies on one of the solid's. A convex edge's is cut away, a concave edge's
/// added. `run_on` says, per end, whether it may continue past the end into air.
pub fn edge_solid(edge: &FeatureEdge, treatment: Treatment, run_on: [bool; 2]) -> Option<Mesh> {
    edge_solid_ends(edge, treatment, run_on.map(|on| if on { End::RunOn } else { End::Stop }))
}

/// The same, finished at each end as `ends` says ([`edge_ends`]).
pub fn edge_solid_ends(edge: &FeatureEdge, treatment: Treatment, ends: [End; 2]) -> Option<Mesh> {
    let profile = edge_profile(edge, treatment)?;
    let sign = if edge.convex { 1.0 } else { -1.0 };
    let [n1, n2] = edge.normals.map(|n| n * sign);
    let reach = treatment.reach(edge.opening());
    let margin = reach * 0.5 + 0.05;
    let first = *profile.curve.first()?;
    let last = *profile.curve.last()?;
    let mut ring = vec![first + n1 * margin, (n1 + n2).normalized() * margin * 1.5, last + n2 * margin];
    ring.extend(profile.curve.iter().rev().copied());

    let (p, q) = (edge.along[0], edge.direction().cross(edge.along[0]));
    let flat: Vec<[f64; 2]> = ring.iter().map(|v| [v.dot(p), v.dot(q)]).collect();
    let outline = Outline::new(flat, Vec::new())?;
    Some(sweep(edge, &outline, ends, reach + margin))
}

/// Whether each end of `edge` may run on into what lies beyond it: only a convex edge's, and only
/// where `solid` has no material just past the end, so the cutter cannot bite into a face that
/// carries on there. A concave edge's fill never runs on; past its end is air it would build into.
pub fn run_on(solid: &Mesh, edge: &FeatureEdge, treatment: Treatment) -> [bool; 2] {
    if !edge.convex {
        return [false, false];
    }
    let probe = treatment.reach(edge.opening()).min(edge.length()) * 0.25;
    let inward = (edge.along[0] + edge.along[1]).normalized() * probe;
    let t = edge.direction();
    [(edge.a, -t), (edge.b, t)].map(|(end, out)| !super::contains_point(solid, end + out * probe + inward))
}

/// The unit vector a fraction `s` of the way from `a` to `b` along the short arc between them.
fn slerp(a: Vec3, b: Vec3, s: f64) -> Vec3 {
    let angle = a.dot(b).clamp(-1.0, 1.0).acos();
    if angle < 1e-9 {
        return a;
    }
    let sin = angle.sin();
    (a * (((1.0 - s) * angle).sin() / sin) + b * ((s * angle).sin() / sin)).normalized()
}

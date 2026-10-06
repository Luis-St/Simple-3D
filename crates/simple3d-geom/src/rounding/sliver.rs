//! The material a treatment takes away or adds, exactly, for showing it before it is made. The
//! cutters of [`edge_solid`] and [`corner_round`] overshoot into the air so their booleans are clean;
//! these stop at the faces, so drawn they look like what will go or come.

use super::corner::corner_ball;
use super::mitre::{sweep, End};
use super::*;
use crate::mesh::Mesh;
use crate::push_pull::Outline;
use crate::vec3::Vec3;

/// The sliver between `edge` and its treatment's curve, along the edge and on into a mitre where
/// `ends` says so ([`edge_ends`]).
pub fn edge_sliver(edge: &FeatureEdge, treatment: Treatment, ends: [End; 2]) -> Option<Mesh> {
    let profile = edge_profile(edge, treatment)?;
    // The edge itself, then the curve back from the second face to the first: the same turn as the
    // cutter's ring, with the edge where the cutter's overshoot was.
    let mut ring = vec![Vec3::ZERO];
    ring.extend(profile.curve.iter().rev().copied());
    let (p, q) = (edge.along[0], edge.direction().cross(edge.along[0]));
    let flat: Vec<[f64; 2]> = ring.iter().map(|v| [v.dot(p), v.dot(q)]).collect();
    let outline = Outline::new(flat, Vec::new())?;
    // The cutter runs on into the air past an open end, where there is nothing to take away; only a
    // mitre carries the sliver past the end, into the corner it shares.
    let ends = ends.map(|end| if end == End::RunOn { End::Stop } else { end });
    Some(sweep(edge, &outline, ends, treatment.reach(edge.opening())))
}

/// The tip a treatment takes off `corner`: up to the ball for a round, the plane for a chamfer.
pub fn corner_sliver(corner: &Corner, treatment: Treatment) -> Option<Mesh> {
    match treatment {
        Treatment::Round { radius, segments } => corner_ball(corner, radius, segments, 0.0),
        Treatment::Chamfer { distance } => {
            if corner.edges.iter().any(|&(_, length)| length <= distance) {
                return None;
            }
            let mut points: Vec<Vec3> = corner.edges.iter().map(|&(dir, _)| corner.at + dir * distance).collect();
            points.push(corner.at);
            Some(crate::hull::convex_hull(&points))
        }
    }
}

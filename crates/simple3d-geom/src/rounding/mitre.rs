//! How each end of an edge's solid finishes: flat at the end, run on into the air past it, or mitred
//! against another treated edge meeting it at an inside corner.
//!
//! Two outside edges of a top face meeting where the solid turns inward (the inner corner of an L)
//! cannot run on, past either end is material, and stopping both flat leaves a spike of the corner
//! standing between their rounds. Run on to the plane bisecting the two edges, their rounds meet
//! there in the seam a picture frame's inside corner has.

use super::*;
use crate::mesh::Mesh;
use crate::push_pull::{extrude_outline, Outline};
use crate::vec3::Vec3;

/// How one end of an edge's solid finishes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End {
    /// Flat, square to the edge, at its end.
    Stop,
    /// On past the end into the air beyond it.
    RunOn,
    /// On past the end up to the plane through it with this normal, which points away from the edge.
    Mitre(Vec3),
}

/// How the ends of `edges[index]` finish on `solid`, with `edges` the other edges treated with it.
/// Only at the inside corners in `joints` ([`inside_corners`], picked) are two edges mitred.
pub fn edge_ends(solid: &Mesh, edges: &[FeatureEdge], index: usize, treatment: Treatment, joints: &[Vec3]) -> [End; 2] {
    let edge = &edges[index];
    let run = run_on(solid, edge, treatment);
    let t = edge.direction();
    [(0, edge.a, -t), (1, edge.b, t)].map(|(side, joint, out)| {
        if run[side] {
            return End::RunOn;
        }
        if !edge.convex || !joints.iter().any(|&j| near(j, joint)) {
            return End::Stop;
        }
        let meeting = edges.iter().enumerate().find_map(|(j, other)| {
            if j == index || !other.convex {
                return None;
            }
            let (into, other_side) = if near(other.a, joint) {
                (other.direction(), 0)
            } else if near(other.b, joint) {
                (-other.direction(), 1)
            } else {
                return None;
            };
            // In line, it is the same edge carried on; on no shared face, the edges do not frame one.
            let shares = other.normals.iter().any(|m| edge.normals.iter().any(|n| n.dot(*m) > 1.0 - 1e-6));
            let inside = !run_on(solid, other, treatment)[other_side];
            (into.dot(out).abs() < 1.0 - 1e-6 && shares && inside).then_some(into)
        });
        match meeting {
            Some(into) => End::Mitre((out + into).normalized()),
            None => End::Stop,
        }
    })
}

/// Every point where two outside edges of one face meet at an angle and the solid turns inward
/// between them: the inner corner of an L, which can be picked to mitre the two there.
pub fn inside_corners(edges: &[FeatureEdge]) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::new();
    for (i, edge) in edges.iter().enumerate() {
        for joint in [edge.a, edge.b] {
            if out.iter().any(|&p| near(p, joint)) {
                continue;
            }
            let at: Vec<&FeatureEdge> = edges.iter().filter(|e| near(e.a, joint) || near(e.b, joint)).collect();
            // A convex corner has only outside edges; an inside one has an inward edge too.
            let inward = at.iter().any(|e| !e.convex);
            let framing = edges.iter().enumerate().any(|(j, other)| {
                j != i
                    && edge.convex
                    && other.convex
                    && (near(other.a, joint) || near(other.b, joint))
                    && other.direction().dot(edge.direction()).abs() < 1.0 - 1e-6
                    && other.normals.iter().any(|m| edge.normals.iter().any(|n| n.dot(*m) > 1.0 - 1e-6))
            });
            if inward && framing {
                out.push(joint);
            }
        }
    }
    out
}

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-6
}

/// Two solids mitred against each other end on the same plane, so they are to be joined before they
/// are used: cut from a model one after the other, their faces on it would be left torn there.
///
/// `outline`, in the edge's cross-section frame (along its first face, and square to that), swept
/// along `edge` and finished at each end as `ends` says. `extend` is how far past an end it may go.
pub(super) fn sweep(edge: &FeatureEdge, outline: &Outline, ends: [End; 2], extend: f64) -> Mesh {
    let t = edge.direction();
    let p = edge.along[0];
    let q = t.cross(p);
    let [before, after] = ends.map(|end| if end == End::Stop { 0.0 } else { extend });
    let length = edge.length() + before + after;
    let start = edge.a - t * before;
    let middle = before + edge.length() / 2.0;
    let mut mesh = extrude_outline(outline, length);
    for v in mesh.positions.iter_mut() {
        let across = p * v.x + q * v.y;
        // The caps are the only vertices, at either end of the sweep.
        let (end, joint, range) =
            if v.z < middle { (ends[0], edge.a, 0.0..=middle) } else { (ends[1], edge.b, middle..=length) };
        let mut s = v.z;
        if let End::Mitre(normal) = end {
            let along = normal.dot(t);
            if along.abs() > 1e-9 {
                s = (normal.dot(joint - start - across) / along).clamp(*range.start(), *range.end());
            }
        }
        *v = start + across + t * s;
    }
    // The frame is right-handed and a mitre only slides each cap along the edge, so the winding is
    // still outward.
    mesh
}

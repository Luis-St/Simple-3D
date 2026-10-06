//! Corners where three or more sharp edges meet: blended into a ball, or cut off by a plane.

use super::*;
use crate::mesh::{FastMap, Mesh};
use crate::vec3::Vec3;
use serde::{Deserialize, Serialize};

/// A convex corner of a solid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Corner {
    pub at: Vec3,
    /// The edges meeting here, as unit directions away from the corner, with their full lengths.
    pub edges: Vec<(Vec3, f64)>,
    /// The outward normals of the faces meeting here, each once.
    pub faces: Vec<Vec3>,
    /// The source node of the first edge's first face.
    #[serde(skip)]
    pub source: u32,
}

impl Corner {
    /// Every corner of `edges` with three or more convex edges and no concave one.
    pub fn find(edges: &[FeatureEdge]) -> Vec<Corner> {
        let key = |p: Vec3| ((p.x * 1e5).round() as i64, (p.y * 1e5).round() as i64, (p.z * 1e5).round() as i64);
        let mut at: FastMap<(i64, i64, i64), Vec<(usize, bool)>> = FastMap::default();
        for (i, e) in edges.iter().enumerate() {
            at.entry(key(e.a)).or_default().push((i, true));
            at.entry(key(e.b)).or_default().push((i, false));
        }
        let mut corners: Vec<Corner> = at
            .values()
            .filter(|list| list.len() >= 3 && list.iter().all(|&(i, _)| edges[i].convex))
            .map(|list| {
                let (first, from_a) = list[0];
                let point = if from_a { edges[first].a } else { edges[first].b };
                let mut faces: Vec<Vec3> = Vec::new();
                for &(i, _) in list {
                    for n in edges[i].normals {
                        if !faces.iter().any(|f| f.dot(n) > 1.0 - 1e-6) {
                            faces.push(n);
                        }
                    }
                }
                let edges_here = list
                    .iter()
                    .map(|&(i, from_a)| {
                        let dir = if from_a { edges[i].direction() } else { -edges[i].direction() };
                        (dir, edges[i].length())
                    })
                    .collect();
                Corner { at: point, edges: edges_here, faces, source: edges[first].sources[0] }
            })
            .collect();
        corners.sort_by_key(|c| key(c.at));
        corners
    }
}

/// The cutter that blends a corner of three faces into a ball of `radius`, matching edges rounded to
/// the same radius: the block between the faces and the ball's tangent planes, less the ball.
/// `None` unless exactly three faces meet there.
pub fn corner_round(corner: &Corner, radius: f64, segments: u32) -> Option<Mesh> {
    corner_ball(corner, radius, segments, radius * 0.5 + 0.05)
}

/// The block between the faces grown out by `margin` and the ball's tangent planes, less the ball.
pub(super) fn corner_ball(corner: &Corner, radius: f64, segments: u32, margin: f64) -> Option<Mesh> {
    let [a, b, c] = corner.faces[..] else { return None };
    let normals = [a, b, c];
    // The ball's centre is `radius` inside each face.
    let centre = solve(normals, normals.map(|n| n.dot(corner.at) - radius))?;
    let mut points = Vec::with_capacity(8);
    for bits in 0..8u32 {
        let offsets = [0, 1, 2].map(|i| {
            if bits & (1 << i) == 0 {
                normals[i].dot(centre)
            } else {
                normals[i].dot(corner.at) + margin
            }
        });
        points.push(solve(normals, offsets)?);
    }
    let block = crate::hull::convex_hull(&points);
    let ball = crate::primitives::ellipsoid_mesh(radius * 2.0, radius * 2.0, radius * 2.0, segments.max(3) * 4)
        .translated(centre);
    Some(crate::csg_bsp::subtract(&block, &ball))
}

/// The cutter that bevels a corner: the tip beyond the plane through the points `distance` along each
/// edge, grown past the faces.
pub fn corner_chamfer(corner: &Corner, distance: f64) -> Option<Mesh> {
    if corner.edges.iter().any(|&(_, length)| length <= distance) {
        return None;
    }
    let base: Vec<Vec3> = corner.edges.iter().map(|&(dir, _)| corner.at + dir * distance).collect();
    let middle = base.iter().fold(Vec3::ZERO, |sum, &p| sum + p) / base.len() as f64;
    let mut points: Vec<Vec3> = base.iter().map(|&p| p + (p - middle) * 0.3).collect();
    points.push(corner.at + (corner.at - middle));
    Some(crate::hull::convex_hull(&points))
}

/// The point `x` with `normals[i] . x = offsets[i]`, by Cramer's rule.
fn solve(normals: [Vec3; 3], offsets: [f64; 3]) -> Option<Vec3> {
    let [a, b, c] = normals;
    let det = a.dot(b.cross(c));
    if det.abs() < 1e-9 {
        return None;
    }
    Some((b.cross(c) * offsets[0] + c.cross(a) * offsets[1] + a.cross(b) * offsets[2]) / det)
}

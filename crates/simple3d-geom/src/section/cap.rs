//! Chaining cut edges into loops and filling them, so a sectioned solid reads as solid.

use super::*;
use crate::mesh::Mesh;
use crate::planar;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// The closed outlines the plane leaves in `mesh`, in world space: outer loops counter-clockwise
/// about the normal from the removed side, holes the other way. `mesh` should be welded so edge
/// crossings chain; unclosed stretches are dropped rather than guessed.
pub fn loops(mesh: &Mesh, plane: &Plane) -> Vec<Vec<Vec3>> {
    tagged_loops(mesh, plane).0
}

/// [`loops`], with the colour tag of the face each outline point was cut from, so a cap can take
/// the colour of the body it cuts through (issue 114).
pub fn tagged_loops(mesh: &Mesh, plane: &Plane) -> (Vec<Vec<Vec3>>, CutTags) {
    let mut segments: Vec<[Vec3; 2]> = Vec::new();
    let mut tags = HashMap::new();
    for (index, tri) in mesh.indices.iter().enumerate() {
        let world = mesh.corners(*tri);
        if let Some(cut) = clip_triangle(plane, world).cut {
            tags.insert(key(cut[0]), mesh.tag(index));
            segments.push(cut);
        }
    }
    (chain(&segments), CutTags(tags))
}

/// The tag of the face under each point of a cut's outlines ([`tagged_loops`]).
pub struct CutTags(HashMap<(i64, i64, i64), u32>);

impl CutTags {
    /// The tag of a cap triangle between outline points: the one most of its corners share, else
    /// its first corner's, so a body's cap takes its colour up to where it meets another's.
    pub fn of_triangle(&self, corners: [Vec3; 3]) -> u32 {
        let [a, b, c] = corners.map(|p| self.0.get(&key(p)).copied().unwrap_or(0));
        if b == c {
            b
        } else {
            a
        }
    }
}

/// A point's bucket for joining segments, at the welder's 1e-6 mm, since shared-edge crossings may
/// differ in the last bit.
pub(crate) fn key(p: Vec3) -> (i64, i64, i64) {
    let s = 1_000_000.0;
    ((p.x * s).round() as i64, (p.y * s).round() as i64, (p.z * s).round() as i64)
}

/// Join cut edges into closed loops, following each end to the edge that starts there.
pub(crate) fn chain(segments: &[[Vec3; 2]]) -> Vec<Vec<Vec3>> {
    let mut starting: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (index, segment) in segments.iter().enumerate() {
        starting.entry(key(segment[0])).or_default().push(index);
    }
    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();
    for first in 0..segments.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let home = key(segments[first][0]);
        let mut points = vec![segments[first][0]];
        let mut at = segments[first][1];
        // A loop cannot have more points than edges, which stops a knotted chain from spinning forever.
        for _ in 0..segments.len() {
            if key(at) == home {
                break;
            }
            let Some(next) = starting.get(&key(at)).and_then(|from| from.iter().copied().find(|&i| !used[i])) else {
                points.clear();
                break;
            };
            used[next] = true;
            points.push(segments[next][0]);
            at = segments[next][1];
        }
        // Only closed chains of at least three points are loops.
        if points.len() >= 3 && key(at) == home {
            out.push(points);
        }
    }
    out
}

/// The cap as triangles facing the removed side. Holes stay holes, so a cut tube shows a ring;
/// an uninterpretable outline leaves that part open rather than guessing.
pub fn cap(mesh: &Mesh, plane: &Plane) -> Vec<[Vec3; 3]> {
    fill(&loops(mesh, plane), plane.normal)
}

/// Fill closed coplanar outlines, for callers that already have them (the renderer draws them too).
pub fn fill(outlines: &[Vec<Vec3>], normal: Vec3) -> Vec<[Vec3; 3]> {
    if outlines.is_empty() {
        return Vec::new();
    }
    let mut positions: Vec<Vec3> = Vec::new();
    let mut indexed: Vec<Vec<u32>> = Vec::new();
    for outline in outlines {
        let start = positions.len() as u32;
        positions.extend(outline.iter().copied());
        indexed.push((0..outline.len() as u32).map(|i| start + i).collect());
    }
    match planar::triangulate_loops(&positions, normal, indexed) {
        Some(triangles) => triangles
            .into_iter()
            .map(|t| [positions[t[0] as usize], positions[t[1] as usize], positions[t[2] as usize]])
            .collect(),
        None => Vec::new(),
    }
}

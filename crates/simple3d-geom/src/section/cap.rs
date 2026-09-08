//! Chaining cut edges into loops and filling them, so a sectioned solid
//! reads as solid rather than hollow.

use super::*;
use crate::mesh::Mesh;
use crate::planar;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// The closed outlines the plane leaves in `mesh`: the shape of the material
/// where it was cut, in world space.
///
/// Each loop is wound counter-clockwise about the plane normal seen from the
/// side that was cut away, and a hole in the material the other way round --
/// the convention a filled outline is read with everywhere in this crate.
///
/// `mesh` should be welded, so that the two triangles sharing an edge compute
/// the same crossing point on it and the segments chain up. A stretch that does
/// not close is dropped rather than guessed at: the cap it would have made is
/// worth less than a wrong one is harmful.
pub fn loops(mesh: &Mesh, plane: &Plane) -> Vec<Vec<Vec3>> {
    let mut segments: Vec<[Vec3; 2]> = Vec::new();
    for tri in &mesh.indices {
        let world = [mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]];
        if let Some(cut) = clip_triangle(plane, world).cut {
            segments.push(cut);
        }
    }
    chain(&segments)
}

/// Buckets a point falls in for the purpose of joining segments end to end, at
/// the same 1e-6 mm the mesh welder uses: two triangles either side of an edge
/// cross the plane at the same place, but not always in the same last bit.
pub(crate) fn key(p: Vec3) -> (i64, i64, i64) {
    let s = 1_000_000.0;
    ((p.x * s).round() as i64, (p.y * s).round() as i64, (p.z * s).round() as i64)
}

/// Join the cut edges into closed loops, following each one from its end to
/// whichever edge starts there.
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
        // A loop cannot be longer than the number of edges there are, and
        // saying so is what keeps a mesh that chains into a knot from spinning
        // here for ever.
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
        // Anything that did not come back to where it started is not a loop,
        // and a loop of two points encloses nothing.
        if points.len() >= 3 && key(at) == home {
            out.push(points);
        }
    }
    out
}

/// The cap: the cut filled in, as triangles facing the side that was cut away.
///
/// Holes come out as holes -- the triangulator reads a loop wound against the
/// normal as one -- so the cap of a tube is a ring and a section through it
/// shows the wall it actually has. An outline the triangulator cannot make
/// sense of contributes nothing rather than a guess, which leaves that part of
/// the cut open: the picture is then what it was before caps existed, which is
/// a fair way to fail.
pub fn cap(mesh: &Mesh, plane: &Plane) -> Vec<[Vec3; 3]> {
    fill(&loops(mesh, plane), plane.normal)
}

/// Fill closed outlines that all lie in one plane. Split out so a caller that
/// already has the outlines -- the renderer draws them as well as fills them --
/// does not find them twice.
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

//! Re-triangulating one flat region from its loops.

use super::*;
use crate::vec3::Vec3;

/// Triangulate closed coplanar loops: outer ones counter-clockwise about `normal`, holes the other
/// way. Public because the section cap uses it and must read holes the same way (issue 71).
pub fn triangulate_loops(positions: &[Vec3], normal: Vec3, loops: Vec<Vec<u32>>) -> Option<Vec<[u32; 3]>> {
    triangulate_region(positions, normal, loops)
}

pub(crate) fn triangulate_region(positions: &[Vec3], normal: Vec3, loops: Vec<Vec<u32>>) -> Option<Vec<[u32; 3]>> {
    let (u, v) = plane_basis(normal);
    triangulate_in(positions, u, v, loops.clone()).or_else(|| {
        let (s, c) = TURNED.to_radians().sin_cos();
        triangulate_in(positions, u * c + v * s, v * c - u * s, loops)
    })
}

/// The angle a region is retried at: bridges from holes' rightmost points lined up with rows of
/// tangent points in a drilled grid and walled the clipper in; turned, they cross the rows.
const TURNED: f64 = 37.0;

fn triangulate_in(positions: &[Vec3], u: Vec3, v: Vec3, loops: Vec<Vec<u32>>) -> Option<Vec<[u32; 3]>> {
    let flatten = |ids: &[u32]| -> Vec<Point> {
        ids.iter().map(|&i| (positions[i as usize].dot(u), positions[i as usize].dot(v))).collect()
    };

    // With the normal's winding, outer loops enclose positive area and holes negative.
    let mut outers: Vec<(Vec<u32>, Vec<Point>, f64)> = Vec::new();
    let mut holes: Vec<(Vec<u32>, Vec<Point>)> = Vec::new();
    for ids in loops {
        let (ids, points) = drop_collinear(&ids, &flatten(&ids))?;
        let area = signed_area(&points);
        if area.abs() < 1e-18 {
            return None; // a degenerate loop; not worth guessing at
        }
        if area > 0.0 {
            outers.push((ids, points, area));
        } else {
            holes.push((ids, points));
        }
    }
    if outers.is_empty() {
        return None;
    }

    // Each hole belongs to the smallest outer loop containing it, since a plane can hold several islands.
    let mut assigned: Vec<Vec<usize>> = vec![Vec::new(); outers.len()];
    for (h, (_, points)) in holes.iter().enumerate() {
        let probe = points[0];
        let owner = outers
            .iter()
            .enumerate()
            .filter(|(_, (_, outer, _))| point_in_polygon(probe, outer))
            .min_by(|(_, a), (_, b)| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)?;
        assigned[owner].push(h);
    }

    let mut out = Vec::new();
    for (i, (ids, points, _)) in outers.iter().enumerate() {
        let (mut ids, mut points) = (ids.clone(), points.clone());
        // Outermost first, so a later bridge can see the seam an earlier one left.
        let mut mine = assigned[i].clone();
        mine.sort_by(|&a, &b| {
            let key = |h: usize| holes[h].1.iter().fold(f64::MIN, |m: f64, p| m.max(p.0));
            key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal)
        });
        for (k, &h) in mine.iter().enumerate() {
            let waiting: Vec<&[Point]> = mine[k + 1..].iter().map(|&w| holes[w].1.as_slice()).collect();
            bridge_hole(&mut ids, &mut points, &holes[h].0, &holes[h].1, &waiting)?;
        }
        ear_clip(&ids, &points, &mut out)?;
    }
    Some(out)
}

/// Drop vertices on the line between their neighbours, so every vertex is a real corner; collinear
/// T-junction vertices stall an ear clipper. `heal` re-splits the edges afterwards. `None` for
/// fewer than three corners.
pub(crate) fn drop_collinear(ids: &[u32], points: &[Point]) -> Option<(Vec<u32>, Vec<Point>)> {
    let n = ids.len();
    // Neighbour comparison suffices even for longer collinear runs: one pass drops them all.
    let corner = |i: usize| {
        let (a, b, c) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        let span = ((c.0 - a.0).powi(2) + (c.1 - a.1).powi(2)).sqrt();
        span > 1e-12 && cross.abs() / span > 1e-9
    };
    let mut kept_ids = Vec::with_capacity(n);
    let mut kept_points = Vec::with_capacity(n);
    for i in 0..n {
        if corner(i) {
            kept_ids.push(ids[i]);
            kept_points.push(points[i]);
        }
    }
    if kept_ids.len() < 3 {
        return None;
    }
    Some((kept_ids, kept_points))
}

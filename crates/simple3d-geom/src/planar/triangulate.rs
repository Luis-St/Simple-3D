//! Re-triangulating one flat region from its loops.

use super::*;
use crate::vec3::Vec3;

/// Triangulate closed loops that lie in one plane: outer boundaries wound
/// counter-clockwise about `normal`, holes wound the other way.
///
/// Public because the section view fills the outlines a cut leaves in the model
/// with exactly this, and a cap that reads a hole differently from the way this
/// pass reads one would show a wall where there is none (issue 71).
pub fn triangulate_loops(positions: &[Vec3], normal: Vec3, loops: Vec<Vec<u32>>) -> Option<Vec<[u32; 3]>> {
    triangulate_region(positions, normal, loops)
}

pub(crate) fn triangulate_region(positions: &[Vec3], normal: Vec3, loops: Vec<Vec<u32>>) -> Option<Vec<[u32; 3]>> {
    let (u, v) = plane_basis(normal);
    let flatten = |ids: &[u32]| -> Vec<Point> {
        ids.iter().map(|&i| (positions[i as usize].dot(u), positions[i as usize].dot(v))).collect()
    };

    // Wound with the surface normal, an outer boundary encloses positive area
    // and a hole encloses negative area.
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

    // One plane can carry several separate islands, so each hole belongs to the
    // smallest outer loop that contains it.
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
        // Outermost first: bridging a hole splices it into the outer loop, and
        // a later bridge has to be able to see the seam the earlier one left.
        let mut mine = assigned[i].clone();
        mine.sort_by(|&a, &b| {
            let key = |h: usize| holes[h].1.iter().fold(f64::MIN, |m: f64, p| m.max(p.0));
            key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal)
        });
        for h in mine {
            bridge_hole(&mut ids, &mut points, &holes[h].0, &holes[h].1)?;
        }
        ear_clip(&ids, &points, &mut out)?;
    }
    Some(out)
}

/// Drop vertices that lie on the straight line between their neighbours, giving
/// the ear clipper a loop whose every vertex is a genuine corner.
///
/// A healed boundary is full of collinear vertices -- that is precisely what
/// [`crate::repair::split_t_junctions`] put there so this face's edges match the
/// neighbouring face's. Feeding them to an ear clipper is what makes it stall:
/// they are never valid ear apexes, so a run of them can be left as the final
/// three vertices with no ear to take. They are not lost by removing them here,
/// because `heal` runs the T-junction pass again afterwards and re-splits the
/// long edges this leaves at exactly the same points.
///
/// Returns `None` for a loop with fewer than three genuine corners: it encloses
/// no area, and guessing at it is worse than leaving the region alone.
pub(crate) fn drop_collinear(ids: &[u32], points: &[Point]) -> Option<(Vec<u32>, Vec<Point>)> {
    let n = ids.len();
    // Comparing against immediate neighbours is enough even for a run of three
    // or more collinear vertices: every vertex strictly inside such a run has
    // two neighbours on the same line, so one pass drops the whole run.
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

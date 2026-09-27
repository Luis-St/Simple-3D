//! Clipping polygons against a convex body, with containment tests that avoid cutting.

use super::*;
use crate::vec3::Vec3;

/// Clip `polygons` against a convex body, keeping what is outside it.
///
/// Same planes and answers as the chain, but only for faces whose boxes come near the polygon
/// (issue B): the surface can only cross a polygon inside a nearby face. A piece in front of a near
/// plane is kept at once; the rest go to `ConvexBody::contains`.
pub(crate) fn clip_convex(
    body: &ConvexBody,
    surface: &BoxTree,
    polygons: Vec<Polygon>,
    scratch: &mut ClipScratch,
    give_up: crate::Abandon<'_>,
) -> Vec<Polygon> {
    let mut kept = Vec::new();
    // Behind every face is inside, which the clip removes, unless the body is inverted.
    let keep_outside = !body.inverted;
    let (mut cf, mut cb, mut front, mut back) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    // Cancellation is checked per batch, since this is the kernel's innermost loop.
    let mut countdown = ABANDON_EVERY;
    for p in polygons {
        countdown -= 1;
        if countdown == 0 {
            countdown = ABANDON_EVERY;
            if give_up() {
                return Vec::new();
            }
        }
        surface.gather(p.bounds, &mut scratch.near, &mut scratch.boxes);
        // By plane in `chain`'s order, so output is deterministic.
        scratch.planes.clear();
        scratch.planes.extend(scratch.near.iter().map(|&i| body.plane_of[i as usize]));
        scratch.planes.sort_unstable();
        scratch.planes.dedup();

        scratch.pieces.clear();
        scratch.pieces.push(p);
        for i in 0..scratch.planes.len() {
            if scratch.pieces.is_empty() {
                break;
            }
            let plane = body.planes[scratch.planes[i] as usize];
            scratch.next.clear();
            for piece in scratch.pieces.drain(..) {
                scratch.splitter.split(&plane, piece, &mut cf, &mut cb, &mut front, &mut back);
                // Coplanar faces go with the plane they face the same way as, keeping one copy of coincident faces.
                front.append(&mut cf);
                back.append(&mut cb);
                if keep_outside {
                    kept.append(&mut front);
                } else {
                    front.clear();
                }
                scratch.next.append(&mut back);
            }
            std::mem::swap(&mut scratch.pieces, &mut scratch.next);
        }
        // Behind every near face: inside unless a far plane says otherwise.
        for piece in scratch.pieces.drain(..) {
            if body.contains(piece.centroid()) != keep_outside {
                kept.push(piece);
            }
        }
    }
    kept
}

/// Whether this convex face covers `point`, assumed in its plane. Generous by `EPSILON`, so a
/// point on a shared edge is covered by both faces rather than neither.
pub(crate) fn covers(face: &Polygon, point: Vec3) -> bool {
    let n = face.vertices.len();
    (0..n).all(|i| {
        let (a, b) = (face.vertices[i], face.vertices[(i + 1) % n]);
        (b - a).cross(point - a).dot(face.plane.normal) >= -EPSILON * (b - a).length()
    })
}

/// The signed form of [`covers`]: positive inside, negative outside, near zero on the rim, for the
/// parity test.
pub(crate) fn covered_by(face: &Polygon, point: Vec3) -> f64 {
    let n = face.vertices.len();
    let mut margin = f64::MAX;
    for i in 0..n {
        let (a, b) = (face.vertices[i], face.vertices[(i + 1) % n]);
        let edge = b - a;
        let length = edge.length();
        if length < 1e-12 {
            continue;
        }
        margin = margin.min(edge.cross(point - a).dot(face.plane.normal) / length);
    }
    margin
}

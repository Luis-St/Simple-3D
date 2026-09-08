//! Clipping a polygon against a convex body, and the containment tests that
//! let the answer be reached without cutting anything.

use super::*;
use crate::vec3::Vec3;

/// Clip `polygons` against a convex body, keeping what is outside it.
///
/// The chain the general path descends asks, of every polygon, "are you in
/// front of *this* plane?" once for each of the body's faces, and answers
/// "kept" the first time a polygon is. That answer is right -- in front of a
/// face plane of a convex solid is outside it -- and the cost of reaching it is
/// the whole of issue B: every fragment is carried past every plane, including
/// the tens of thousands whose faces are nowhere near it.
///
/// This asks the same question of the same planes and reaches the same answer,
/// but only of the faces whose boxes come near the polygon. That is sound
/// rather than approximate: the body's surface is the union of its faces, so
/// wherever the surface crosses the polygon it does so inside one of those
/// faces, and therefore on one of their planes. Cutting by those planes alone
/// leaves pieces whose interiors the surface does not cross, and one point
/// settles each of them.
///
/// Most pieces are settled without that point test at all, by the same rule the
/// chain uses: a piece in front of a near face's plane is outside the body and
/// is kept on the spot. Only what survives every near plane -- the part of the
/// polygon covered by the body, and any polygon that came near no face at all
/// -- is put to `ConvexBody::contains`.
pub(crate) fn clip_convex(
    body: &ConvexBody,
    surface: &BoxTree,
    polygons: Vec<Polygon>,
    scratch: &mut ClipScratch,
    give_up: crate::Abandon<'_>,
) -> Vec<Polygon> {
    let mut kept = Vec::new();
    // Behind every face is inside the body, which the clip removes -- unless
    // the body has been inverted, when inside is what survives.
    let keep_outside = !body.inverted;
    let (mut cf, mut cb, mut front, mut back) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    // Asked once a batch rather than once a polygon: an atomic load is cheap but
    // this is the innermost loop in the kernel, and a batch of this size is
    // microseconds of work.
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
        // By plane rather than by face, in the order `chain` took the planes,
        // so the same input gives the same output every run.
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
                // A coplanar face is kept with the plane it faces the same way
                // as and dropped with the other, exactly as at a chain node --
                // that is what leaves one copy of two coincident faces.
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
        // What no near face could place: behind all of them, so inside the body
        // unless one of the far planes says otherwise.
        for piece in scratch.pieces.drain(..) {
            if body.contains(piece.centroid()) != keep_outside {
                kept.push(piece);
            }
        }
    }
    kept
}

/// Does this face cover `point`, which is taken to lie in its plane?
///
/// Every polygon here is convex, so being inside is being on the inner side of
/// every edge. Generous by `EPSILON`, deliberately: a point on the shared edge
/// of two faces is covered by both, and the two answers agree, where a point
/// this said was covered by neither would fall through to a side test taken on
/// the surface itself.
pub(crate) fn covers(face: &Polygon, point: Vec3) -> bool {
    let n = face.vertices.len();
    (0..n).all(|i| {
        let (a, b) = (face.vertices[i], face.vertices[(i + 1) % n]);
        (b - a).cross(point - a).dot(face.plane.normal) >= -EPSILON * (b - a).length()
    })
}

/// How far inside the face `point` is, measured in its own plane: positive
/// inside, negative outside, and near zero on the rim. The signed form of
/// [`covers`], for the parity test, which has to know when it is passing
/// through an edge rather than through a face.
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

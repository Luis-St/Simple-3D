//! Clipping only what lies near the other body, so a boolean touches the
//! smallest part of each mesh it can.

use super::*;
use crate::vec3::Vec3;

/// Clip `polygons` against a body of any shape, keeping what is outside it --
/// the convex argument of `clip_convex`, made general.
///
/// The classic descent carries every polygon past the plane of every node on
/// its path through the tree, and those planes are infinite. That is what a
/// convex operand made ruinous, and `clip_convex` is what stopped it there; but
/// the *soundness* cost was never confined to the convex case. A plane whose
/// own face is a centimetre away still cuts, and every cut it makes is a
/// corner landing within `EPSILON` of the next such plane -- a grazing sliver
/// the classification then reads as lying *on* a plane it is really just
/// outside, so the plane that should have trimmed it leaves it standing. The
/// triangle between that corner and where the other body's surface really
/// comes down belongs to neither body, nothing emits it, and `heal` cannot
/// close a hole that has no T-junction in it. That is the mechanism the torus
/// pairs kept failing on: not convex, so never reaching the convex clip, and
/// so still being shredded by planes belonging to the far side of the ring.
///
/// The same argument settles it for any body. A body's surface is the union of
/// its faces, so wherever that surface crosses a polygon it crosses inside a
/// face, and therefore on that face's plane -- and a face that crosses the
/// polygon has a box that meets the polygon's. Cutting the polygon by the
/// planes of those faces alone therefore leaves pieces whose interiors the
/// surface does not cross, and each such piece is wholly inside the body or
/// wholly outside it. Convexity was never what made that true; it only bought
/// the extra shortcut of settling a piece the moment it is in front of one
/// plane, which a body with a dent in it cannot claim. Here every undecided
/// piece is put to a point test against the tree instead.
///
/// The coplanar pieces are decided on the spot rather than by that test, since
/// their centroids lie *on* the surface, where a side test has no answer:
/// facing the same way as the body's own face is the outside of it, facing the
/// other way is the inside. That is the rule the classic algorithm applies at
/// the node owning the plane, and it is what leaves exactly one copy of two
/// coincident faces.
pub(crate) fn clip_near(
    root: &BspNode,
    surface: &BoxTree,
    polygons: Vec<Polygon>,
    scratch: &mut ClipScratch,
    give_up: crate::Abandon<'_>,
) -> Vec<Polygon> {
    let face_planes = &root.face_planes;
    let mut kept = Vec::new();
    let (mut cf, mut cb, mut front, mut back) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut countdown = ABANDON_EVERY;
    for p in polygons {
        countdown -= 1;
        if countdown == 0 {
            countdown = ABANDON_EVERY;
            if give_up() {
                return Vec::new();
            }
        }
        // Each piece carries the face it has been cut past, and whichever face
        // it was found to lie in the plane of. The faces are taken in
        // increasing index and never revisited, which is what makes this
        // terminate: a piece is a subset of the piece it was cut from, so a
        // plane the parent lay wholly in front of, wholly behind, or exactly
        // in, the child does too -- there is never a reason to go back.
        scratch.work.clear();
        scratch.work.push((p, 0, None));
        while let Some((piece, from, coplanar)) = scratch.work.pop() {
            // Gathered afresh for every piece, not once for the polygon it came
            // from. That is the whole of it: a plane cuts the piece from edge to
            // edge however small the face it belongs to, so the piece that ends
            // up away from the surface has to stop being offered planes at all,
            // and its own box is what says so. Gathering once and applying the
            // whole list to every piece builds the full arrangement of those
            // planes inside the polygon instead -- thousands of cells across a
            // plate a torus passes through, for a cut that is one curve.
            surface.gather(piece.bounds, &mut scratch.near, &mut scratch.boxes);
            scratch.near.sort_unstable();
            let mut cut = None;
            let mut lies_in = coplanar;
            for &i in scratch.near.iter().filter(|&&i| i >= from) {
                let plane = face_planes[i as usize];
                let (mut lo, mut hi) = (f64::MAX, f64::MIN);
                for v in &piece.vertices {
                    let t = plane.normal.dot(*v) - plane.w;
                    lo = lo.min(t);
                    hi = hi.max(t);
                }
                if lo >= -EPSILON && hi <= EPSILON {
                    // Lying in the face's plane. Noted and passed over rather
                    // than decided: the piece may still straddle the edge of
                    // that face, and it is only once every plane has had its
                    // cut that "in the plane" and "on the face" are the same
                    // question.
                    lies_in = lies_in.or(Some(i));
                    continue;
                }
                if lo < -EPSILON && hi > EPSILON {
                    cut = Some((i, plane));
                    break;
                }
            }
            match cut {
                Some((i, plane)) => {
                    scratch.splitter.split(&plane, piece, &mut cf, &mut cb, &mut front, &mut back);
                    cf.clear();
                    cb.clear();
                    for piece in front.drain(..).chain(back.drain(..)) {
                        scratch.work.push((piece, i + 1, lies_in));
                    }
                }
                None => {
                    // No face of the body crosses this piece any more, so the
                    // surface does not either: it is wholly on one side, and
                    // one point settles the whole of it.
                    let centre = piece.centroid();
                    // Which face the piece is lying *on* -- not merely which one
                    // it shares a plane with. One face of a body is very often
                    // several triangles in the one plane: a box's side wall is
                    // two, and a piece resting on that wall lies in the plane of
                    // both and on only one of them. Taking the first found gave
                    // the coincident-face rule the other triangle's answer,
                    // which is no answer at all -- so the rule fell through to a
                    // side test on a point that is *on* the surface, and both
                    // copies of a shared wall survived. That is the union of a
                    // box with a mesh of several separate boxes coming back
                    // non-manifold.
                    let on = lies_in.filter(|&i| covers(&root.faces[i as usize], centre)).or_else(|| {
                        scratch.near.iter().copied().find(|&j| {
                            let plane = face_planes[j as usize];
                            piece.vertices.iter().all(|v| (plane.normal.dot(*v) - plane.w).abs() <= EPSILON)
                                && covers(&root.faces[j as usize], centre)
                        })
                    });
                    let keep = match on {
                        // On the body's own surface, where a side test has no
                        // answer: facing the same way as the face is the
                        // outside of it, the other way is the inside. This is
                        // the rule the classic algorithm applies at the node
                        // owning the plane, and with the second clip either
                        // side of an `invert` it is what leaves one copy of two
                        // coincident faces rather than two or none.
                        Some(i) => face_planes[i as usize].normal.dot(piece.plane.normal) > 0.0,
                        // In the plane of a face but off the end of it, which
                        // says nothing about the body at all.
                        None => root.keeps_point(centre, &mut scratch.ray, &mut scratch.ray_stack),
                    };
                    if keep {
                        kept.push(piece);
                    }
                }
            }
        }
    }
    kept
}

/// How close to a face's rim a ray may pass before the count it is taking part
/// in is abandoned for another direction. Well under `EPSILON`, which is what
/// decides whether a polygon is *on* a plane at all: a crossing this close to an
/// edge is one the parity test cannot be trusted about, not one that is wrong.
pub(crate) const RAY_EPSILON: f64 = 1e-9;

/// Directions for the parity ray, tried in order until one gives a clean count.
///
/// Skew on purpose, and to no round fraction: the bodies this is asked about are
/// full of axis-aligned faces and axis-aligned edges, and a ray along an axis
/// meets those edge-on. Their length is immaterial: only the direction, and the
/// sign of the `t` it produces, are used.
pub(crate) const RAY_DIRECTIONS: [Vec3; 4] = [
    Vec3 { x: 0.5628, y: 0.6567, z: 0.5019 },
    Vec3 { x: -0.7237, y: 0.4265, z: 0.5427 },
    Vec3 { x: 0.4109, y: -0.7583, z: 0.5065 },
    Vec3 { x: 0.3181, y: 0.5023, z: -0.8038 },
];

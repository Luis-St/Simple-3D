//! Clipping only what lies near the other body, so a boolean touches as little of each mesh as possible.

use super::*;
use crate::vec3::Vec3;

/// Clip `polygons` against a body of any shape, keeping what is outside it; `clip_convex`
/// generalised.
///
/// The classic descent cuts polygons by infinite planes of far-away faces, leaving grazing
/// slivers within `EPSILON` that are misclassified and leave unhealable holes (the torus pair
/// failures). Here a piece is cut only by the planes of faces whose boxes meet it; the resulting
/// pieces are not crossed by the surface, so each is wholly inside or outside and is settled by a
/// point test against the tree.
///
/// Coplanar pieces are decided on the spot, since their centroid lies on the surface: facing the
/// same way as the body's face is outside, the other way inside. This keeps exactly one copy of
/// two coincident faces.
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
        // Each piece carries the face index it has been cut past and any face it lies in. Faces are
        // visited in increasing order and never revisited; a child piece inherits its parent's side of
        // every earlier plane, so this terminates.
        scratch.work.clear();
        scratch.work.push((p, 0, None));
        while let Some((piece, from, coplanar)) = scratch.work.pop() {
            // Gathered per piece rather than per polygon: a piece away from the surface must stop being
            // offered planes. Applying one list to every piece built the full plane arrangement, thousands
            // of cells for a single cut curve.
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
                    // In the face's plane: noted but undecided, since the piece may still straddle the face's edge.
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
                    // No face crosses this piece any more, so one point settles it.
                    let centre = piece.centroid();
                    // The face the piece lies on, not merely shares a plane with. A flat side is often several
                    // triangles; taking the first found gave a side test on a surface point, and both copies of a
                    // shared wall survived, making a union non-manifold.
                    let on = lies_in.filter(|&i| covers(&root.faces[i as usize], centre)).or_else(|| {
                        scratch.near.iter().copied().find(|&j| {
                            let plane = face_planes[j as usize];
                            piece.vertices.iter().all(|v| (plane.normal.dot(*v) - plane.w).abs() <= EPSILON)
                                && covers(&root.faces[j as usize], centre)
                        })
                    });
                    let keep = match on {
                        // On the body's surface: facing the same way is outside, the other way inside. With the second
                        // clip either side of an `invert`, this keeps one copy of two coincident faces.
                        Some(i) => face_planes[i as usize].normal.dot(piece.plane.normal) > 0.0,
                        // In a face's plane but off its end, which says nothing about the body.
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

/// How close to a face's rim a parity ray may pass before trying another direction. Well under
/// `EPSILON`: such a crossing is untrustworthy, not wrong.
pub(crate) const RAY_EPSILON: f64 = 1e-9;

/// Parity ray directions, tried in order until one gives a clean count. Deliberately skew, since
/// the bodies are full of axis-aligned faces and edges. Only direction and the sign of `t` matter.
pub(crate) const RAY_DIRECTIONS: [Vec3; 4] = [
    Vec3 { x: 0.5628, y: 0.6567, z: 0.5019 },
    Vec3 { x: -0.7237, y: 0.4265, z: 0.5427 },
    Vec3 { x: 0.4109, y: -0.7583, z: 0.5065 },
    Vec3 { x: 0.3181, y: 0.5023, z: -0.8038 },
];

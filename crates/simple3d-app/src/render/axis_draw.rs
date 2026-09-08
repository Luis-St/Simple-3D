//! Drawing one origin axis.

use super::*;
use crate::raster::Rgba;
use crate::view::View;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

/// What is left of a stretch of material along `axis` once the section has had
/// it, as the coordinate along that axis.
///
/// The axis runs through the origin, so a point on it is `t` along the axis and
/// zero elsewhere and the plane's own test comes down to one number: the axis
/// crosses it where the normal's component along the axis carries it there, and
/// an axis lying *in* the plane is either wholly kept or wholly gone.
pub(crate) fn trim_span(span: (f64, f64), axis: usize, plane: &Plane) -> Option<(f64, f64)> {
    let slope = component(plane.normal, axis);
    let (lo, hi) = (span.0.min(span.1), span.0.max(span.1));
    let (lo, hi) = if slope > 1e-12 {
        (lo, hi.min(plane.offset / slope))
    } else if slope < -1e-12 {
        (lo.max(plane.offset / slope), hi)
    } else if plane.depth(Vec3::ZERO) <= 0.0 {
        (lo, hi)
    } else {
        return None;
    };
    (hi - lo > 1e-9).then_some((lo, hi))
}

/// One arm of an axis: faded along its length like the grid, and drawn over the
/// frame rather than tested against it -- except where it runs inside material,
/// which is not drawn at all.
///
/// The depth test is the wrong question for an axis. Asked of the depth buffer,
/// an axis disappears wherever the shape merely *stands in front of it*, which
/// is most of the screen once the camera is close: the arm leaving a box at the
/// origin is outside the box from the surface onwards, but its projection stays
/// over the box for a long way, so the line arriving at the shape was missing
/// and only a mark on the face was left (issue 47). Asked of the model instead
/// -- is this stretch of the line inside anything? -- the answer is the one the
/// picture wants: the line runs unbroken up to the surface it goes into, stops
/// there, and picks up again where it comes out (issues 20, 36, 47).
///
/// That exception is the *approach*, and nothing else. It is granted to the
/// stretch of the line on the eye's side of the material, because the stretch
/// beyond the far surface really is behind the shape: drawing it over the solid
/// as well put the line on the face of a box it had already left, which reads
/// as an axis running inside the object instead of out the back of it. So every
/// piece asks the question for itself, and the arm going away is
/// depth-tested like anything else -- hidden by the box, and picked up again
/// where it comes out past the silhouette.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_axis_line(
    out: &mut Vec<AxisStep>,
    view: &View,
    centre: Vec3,
    to: Vec3,
    reach: f64,
    colour: Rgba,
    axis: usize,
    inside: &[(f64, f64)],
    through: &[(f64, f64, u16)],
    tags: usize,
) {
    // Which way depth runs along this axis: positive when travelling along
    // +axis moves away from the eye.
    let away = component(view.forward(), axis);
    let mut seen = vec![false; tags + 1];
    for step in 0..FADE_STEPS {
        let t0 = step as f64 / FADE_STEPS as f64;
        let t1 = (step + 1) as f64 / FADE_STEPS as f64;
        let a = centre + (to - centre) * t0;
        let b = centre + (to - centre) * t1;
        let mid = (a + b) * 0.5;
        let fade = 1.0 - ((mid - centre).length() / (reach * 0.8)).min(1.0).powi(2);
        if fade <= 0.03 {
            continue;
        }
        let faded = [colour[0], colour[1], colour[2], (colour[3] as f64 * fade).round() as u8];
        for (from, to, material) in clip_spans(component(a, axis), component(b, axis), inside) {
            if material {
                continue;
            }
            let (start, end) = (along(axis, from), along(axis, to));
            let (va, vb) = (to_vertex(view, view.to_view(start)), to_vertex(view, view.to_view(end)));
            seen_through(&mut seen, through, (from + to) / 2.0, away);
            out.push(AxisStep { a: va, b: vb, colour: faded, seen: seen.clone().into() });
        }
    }
}

/// Fill `seen`, indexed by body tag, with the bodies that may not hide the piece
/// of an axis at `at` along it.
///
/// A body qualifies only when that piece is on the eye's side of the stretch the
/// axis runs through it: the line is being drawn over the shape so that it can
/// be seen *arriving* at the surface it enters, and past the far surface there
/// is no arrival left to show -- only a solid the line is genuinely behind.
/// `away` is how depth runs along the axis; when it is about zero the axis lies
/// in the screen plane, the two sides are the same distance off, and the
/// exception applies to both.
pub(crate) fn seen_through(seen: &mut [bool], through: &[(f64, f64, u16)], at: f64, away: f64) {
    seen.fill(false);
    for &(lo, hi, tag) in through {
        let Some(slot) = seen.get_mut(tag as usize) else { continue };
        *slot |= if away > 1e-9 {
            at <= lo
        } else if away < -1e-9 {
            at >= hi
        } else {
            true
        };
    }
}

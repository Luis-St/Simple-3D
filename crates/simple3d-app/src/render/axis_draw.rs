//! Drawing one origin axis.

use super::*;
use crate::raster::Rgba;
use crate::view::View;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

/// What is left of a material span along `axis` after the section, as axis coordinates. The axis
/// passes through the origin, so the plane test reduces to one number; an axis lying in the plane
/// is kept or removed whole.
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

/// [`trim_span`] for several cuts: a windowed cut can remove a span's middle and keep both ends.
pub(crate) fn trim_spans(span: (f64, f64), axis: usize, cuts: &[Plane]) -> Vec<(f64, f64)> {
    if let [plane] = cuts {
        if plane.window.is_none() {
            return trim_span(span, axis, plane).into_iter().collect();
        }
    }
    let (lo, hi) = (span.0.min(span.1), span.0.max(span.1));
    let along = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)][axis.min(2)];
    simple3d_geom::section::kept_by_all(cuts, along * lo, along * hi)
        .iter()
        .map(|&(a, b)| (component(a, axis), component(b, axis)))
        .filter(|(a, b)| b - a > 1e-9)
        .collect()
}

/// One axis arm: faded along its length and drawn over the frame, except inside material.
///
/// A plain depth test hides the arm wherever a shape stands in front of it, losing the approach to
/// the surface it enters (issue 47); testing against material instead draws it up to the surface
/// and from where it exits (issues 20, 36, 47). The exception is only for the approach: the stretch
/// beyond the far surface is behind the shape and is depth-tested like anything else.
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
    // Positive when travelling along +axis moves away from the eye.
    let away = component(view.forward(), axis);
    let extents = body_extents(through, tags);
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
            seen_through(&mut seen, &extents, (from + to) / 2.0, away);
            out.push(AxisStep { a: va, b: vb, colour: faded, seen: seen.clone().into() });
        }
    }
}

/// Each body's reach along the axis, by tag: from the first surface met to the last. The approach
/// is judged against the whole body, or a stretch in a drilled hole counts as arriving and is drawn
/// over the near wall.
pub(crate) fn body_extents(through: &[(f64, f64, u16)], tags: usize) -> Vec<Option<(f64, f64)>> {
    let mut extents: Vec<Option<(f64, f64)>> = vec![None; tags + 1];
    for &(lo, hi, tag) in through {
        let Some(slot) = extents.get_mut(tag as usize) else { continue };
        *slot = Some(slot.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
    }
    extents
}

/// Mark in `seen`, by tag, the bodies that may not hide the axis piece at `at`: those the piece is
/// entirely on the eye's side of ([`body_extents`]). `away` is the depth direction along the axis;
/// near zero, either end may be the approach.
pub(crate) fn seen_through(seen: &mut [bool], extents: &[Option<(f64, f64)>], at: f64, away: f64) {
    for (slot, extent) in seen.iter_mut().zip(extents) {
        *slot = extent.is_some_and(|(lo, hi)| {
            if away > 1e-9 {
                at <= lo
            } else if away < -1e-9 {
                at >= hi
            } else {
                at <= lo || at >= hi
            }
        });
    }
}

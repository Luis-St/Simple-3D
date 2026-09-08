//! The grips the section plane is taken hold of by.

use super::*;
use crate::panel_properties::{component, set_component};
use simple3d_core::scene::SectionView;
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};

/// Everything about the section that changes the picture, so the viewport
/// redraws when the plane moves and not when anything else does.
pub fn hash_section(section: &SectionView, hasher: &mut impl Hasher) {
    section.enabled.hash(hasher);
    section.axis().hash(hasher);
    section.offset.to_bits().hash(hasher);
    section.flipped.hash(hasher);
}

/// The four corners of the frame, in world space and in order around it.
///
/// It is sized and centred on the model rather than on the origin: a plane
/// through a part that sits 300 mm out would otherwise be drawn in the middle
/// of the grid, nowhere near the thing it is cutting.
pub fn frame(section: &SectionView, bounds: Option<(Vec3, Vec3)>) -> [Vec3; 4] {
    let axis = section.axis();
    // The two directions the plane spans, with the second one upright wherever
    // the plane is upright: the grip hangs off that edge, and on a standing
    // plane it belongs at the top of it rather than off one side.
    let (u, v) = match axis {
        1 => (0, 2),
        _ => ((axis + 1) % 3, (axis + 2) % 3),
    };
    let (lo, hi) = bounds.unwrap_or((Vec3::splat(-EMPTY_HALF), Vec3::splat(EMPTY_HALF)));
    let half = |a: usize| (((component(hi, a) - component(lo, a)) * 0.5) * (1.0 + MARGIN)).max(MIN_HALF);
    let middle = |a: usize| (component(hi, a) + component(lo, a)) * 0.5;
    let corner = |su: f64, sv: f64| {
        let mut at = Vec3::ZERO;
        set_component(&mut at, axis, section.offset);
        set_component(&mut at, u, middle(u) + su * half(u));
        set_component(&mut at, v, middle(v) + sv * half(v));
        at
    };
    [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)]
}

/// The five places the plane can be taken hold of: the middle of each of the
/// frame's four edges, and the middle of the plane itself (issue 72).
///
/// All five do the same thing -- slide the plane along its axis -- and they are
/// five rather than one because a single grip is only ever in reach from the
/// side of the model it was put on. Orbit round to look at the cut from
/// underneath and a grip on the upper edge is behind the shape, which leaves
/// the plane movable only by the number in its window.
///
/// The middle one is the one place two controls want the same pixels, and it
/// keeps them: [`interact`] is offered the pointer before the manipulator is,
/// so a press at the middle of the plane slides the plane. That costs the
/// selection nothing, because a manipulator handle only lands at the middle of
/// the shape on screen when it is pointing at the camera -- an arrow, a face or
/// a ring seen end-on, which cannot be dragged in that view whoever gets the
/// press. The one it does cost is a ring seen *nearly* edge-on, which passes
/// close to the middle and is still turnable: it is grabbed anywhere else along
/// its length instead.
pub fn grips(corners: &[Vec3; 4]) -> [Vec3; 5] {
    let middle = |a: usize, b: usize| (corners[a] + corners[b]) * 0.5;
    [middle(0, 1), middle(1, 2), middle(2, 3), middle(3, 0), middle(0, 2)]
}

/// The way the plane travels, which is always the positive axis: the offset is
/// a coordinate, so flipping which side is cut away must not turn the numbers
/// round as well.
pub(crate) fn travel(axis: usize) -> Vec3 {
    let mut dir = Vec3::ZERO;
    set_component(&mut dir, axis, 1.0);
    dir
}

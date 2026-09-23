//! The grips the section plane is taken hold of by.

use super::*;
use crate::panel_properties::component;
use simple3d_core::scene::SectionView;
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};

/// Everything about the section that changes the picture, so the viewport
/// redraws when the plane moves and not when anything else does.
pub fn hash_section(section: &SectionView, hasher: &mut impl Hasher) {
    section.enabled.hash(hasher);
    section.axis().hash(hasher);
    section.offset.to_bits().hash(hasher);
    section.keep.hash(hasher);
    section.swept_up.hash(hasher);
    section.custom_size.hash(hasher);
    for side in section.size {
        side.to_bits().hash(hasher);
    }
    for turn in section.tilt {
        turn.to_bits().hash(hasher);
    }
}

/// The four corners of the frame, in world space and in order around it.
///
/// It is sized and centred on the model rather than on the origin: a plane
/// through a part that sits 300 mm out would otherwise be drawn in the middle
/// of the grid, nowhere near the thing it is cutting. A plane given a size of
/// its own is drawn at that size instead, and the frame is then exactly the
/// rectangle that cuts.
///
/// A tilted plane is the untilted frame turned about the middle of the model,
/// the same turn [`SectionView::anchor`] gives the plane itself, so the frame
/// stays in it.
pub fn frame(section: &SectionView, bounds: Option<(Vec3, Vec3)>) -> [Vec3; 4] {
    let (u, v) = section.basis();
    let [width, height] = match section.custom_size {
        true => section.size,
        false => auto_size(section, bounds),
    };
    let (hu, hv) = (width.max(0.0) * 0.5, height.max(0.0) * 0.5);
    let middle = section.anchor(bounds);
    let corner = |su: f64, sv: f64| middle + u * (su * hu) + v * (sv * hv);
    [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)]
}

/// The width and height the frame is drawn at while the plane runs through the
/// whole model: a little wider than the model along each direction it spans.
///
/// Turned, the plane crosses the model at a slant, and a frame sized to the two
/// extents it was standing across can fall short of the shape: it is made
/// square at the largest of the three instead.
pub fn auto_size(section: &SectionView, bounds: Option<(Vec3, Vec3)>) -> [f64; 2] {
    let axis = section.axis();
    let (u, v) = match axis {
        1 => (0, 2),
        _ => ((axis + 1) % 3, (axis + 2) % 3),
    };
    let (lo, hi) = bounds.unwrap_or((Vec3::splat(-EMPTY_HALF), Vec3::splat(EMPTY_HALF)));
    let extent = |a: usize| (((component(hi, a) - component(lo, a)) * 0.5) * (1.0 + MARGIN)).max(MIN_HALF);
    let widest = extent(0).max(extent(1)).max(extent(2));
    let half = |a: usize| if section.tilted() { widest } else { extent(a) };
    [half(u) * 2.0, half(v) * 2.0]
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

/// The way the plane travels: along its own normal, turned with it, and
/// never reversed by which side the camera is on -- the offset is a place in
/// the model, so orbiting round it must not turn the numbers round as well.
pub(crate) fn travel(section: &SectionView) -> Vec3 {
    section.normal()
}

//! The grips the section plane is taken hold of by.

use super::*;
use crate::panel_properties::component;
use simple3d_core::scene::SectionView;
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};

/// Hash what about the section changes the picture, so the viewport redraws only when it moves.
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
    section.centre.map(|at| [at.x, at.y, at.z].map(f64::to_bits)).hash(hasher);
}

/// The frame's four corners in world space, in order. Sized and centred on the model (or at the
/// plane's own size), and turned with a tilted plane about its centre, as [`SectionView::anchor`].
pub fn frame(section: &SectionView, bounds: Option<(Vec3, Vec3)>) -> [Vec3; 4] {
    let (u, v) = section.basis();
    let [width, height] = match section.custom_size {
        true => section.size,
        false => auto_size(section, bounds),
    };
    let (hu, hv) = (width.max(0.0) * 0.5, height.max(0.0) * 0.5);
    let middle = match section.custom_size {
        true => section.anchor(bounds),
        false => section.frame_middle(bounds),
    };
    let corner = |su: f64, sv: f64| middle + u * (su * hu) + v * (sv * hv);
    [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)]
}

/// The frame size while the plane runs through the whole model: a little wider than the model,
/// square at the largest extent when tilted so it never falls short.
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

/// The five grips (edge middles and centre, issue 72), so one is reachable from any orbit. The
/// centre grip is offered the pointer before the manipulator; that only costs nearly edge-on rings,
/// which can be grabbed elsewhere.
pub fn grips(corners: &[Vec3; 4]) -> [Vec3; 5] {
    let middle = |a: usize, b: usize| (corners[a] + corners[b]) * 0.5;
    [middle(0, 1), middle(1, 2), middle(2, 3), middle(3, 0), middle(0, 2)]
}

/// The plane's travel direction: its normal, never flipped by the camera side, since the offset
/// is a place in the model.
pub(crate) fn travel(section: &SectionView) -> Vec3 {
    section.normal()
}

//! Axis-aligned boxes, as `(lo, hi)` corner pairs.

use crate::vec3::Vec3;

/// The box around a set of points, or nothing for no points.
pub fn bounds_of(points: impl IntoIterator<Item = Vec3>) -> Option<(Vec3, Vec3)> {
    let mut it = points.into_iter();
    let first = it.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in it {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    Some((lo, hi))
}

/// Corner `index` (0 to 7) of the box: bits 1, 2 and 4 pick the high X, Y and Z.
pub fn box_corner(lo: Vec3, hi: Vec3, index: usize) -> Vec3 {
    Vec3::new(
        if index & 1 == 0 { lo.x } else { hi.x },
        if index & 2 == 0 { lo.y } else { hi.y },
        if index & 4 == 0 { lo.z } else { hi.z },
    )
}

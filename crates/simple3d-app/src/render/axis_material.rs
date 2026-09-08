//! What an origin axis is drawn like where it is buried in the model.

use super::*;
use simple3d_geom::section::Plane;

/// What the axes meet in the model: where they run inside it, which solids they
/// go through, and how far from the origin the model reaches.
pub(crate) struct AxisMaterial {
    /// Per axis, the stretches inside a solid, as coordinates along that axis.
    pub(super) inside: [Vec<(f64, f64)>; 3],
    /// Per axis, the bodies it runs through: the stretch inside each one, with
    /// that body's tag. Per body and per axis both -- a box the Y axis runs
    /// through is still an ordinary occluder for X and Z, and a box the axes
    /// never touch is an ordinary occluder for all three, however many other
    /// shapes it was merged into one mesh with (img2).
    ///
    /// The span is kept, not just the tag, because *where* a stretch of the
    /// line sits relative to it decides whether that body may hide it: only the
    /// approach, on the eye's side of the material, is drawn over the shape.
    pub(super) through: [Vec<(f64, f64, u16)>; 3],
    /// One past the largest tag in `through`, so a lookup table indexed by tag
    /// can be sized once.
    pub(super) tags: usize,
    /// The distance from the origin to the model's furthest vertex. What an
    /// origin axis's arms have to be longer than, or a shape standing on the
    /// origin holds the whole arm and the axis is never seen at all.
    pub(super) reach: f64,
}

/// Where each item's body tags start, so no two items share one.
pub(crate) fn tag_bases(items: &[Item<'_>]) -> Vec<u16> {
    let mut base = 0_u16;
    items
        .iter()
        .map(|item| {
            let start = base;
            base = base.saturating_add(item.renderable.body_count);
            start
        })
        .collect()
}

pub(crate) fn axis_material(items: &[Item<'_>], grid: &Grid, section: Option<Plane>) -> AxisMaterial {
    let mut material = AxisMaterial {
        inside: [Vec::new(), Vec::new(), Vec::new()],
        through: [Vec::new(), Vec::new(), Vec::new()],
        tags: 0,
        reach: 0.0,
    };
    for (item, tag_base) in items.iter().zip(tag_bases(items)) {
        let positions = item.renderable.mesh.positions.iter();
        material.reach = positions.map(|p| p.length()).fold(material.reach, f64::max);
        // A ghost is see-through, so the axis inside it is too -- and it never
        // reaches the depth buffer either way.
        if item.style != Style::Solid {
            continue;
        }
        for axis in 0..3 {
            if !grid.axes[axis] {
                continue;
            }
            for (span, tag) in axis_inside_spans(item, axis, tag_base) {
                // Material the section took away is no longer in the axis's
                // way: the line is drawn through the space the cut opened, the
                // way it is drawn through empty space anywhere else.
                let Some(span) = section.map_or(Some(span), |plane| trim_span(span, axis, &plane)) else {
                    continue;
                };
                material.inside[axis].push(span);
                material.through[axis].push((span.0, span.1, tag));
                material.tags = material.tags.max(tag as usize + 1);
            }
        }
    }
    material
}

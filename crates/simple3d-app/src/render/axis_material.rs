//! What an origin axis is drawn like where it is buried in the model.

use super::*;
use simple3d_geom::section::Plane;

/// What the axes meet in the model: where they run inside it, which solids they
/// go through, and how far from the origin the model reaches.
pub(crate) struct AxisMaterial {
    /// Per axis, the stretches inside a solid, as coordinates along that axis.
    pub(crate) inside: [Vec<(f64, f64)>; 3],
    /// Per axis, the bodies it runs through: the stretch inside each one, with
    /// that body's tag. Per body and per axis both -- a box the Y axis runs
    /// through is still an ordinary occluder for X and Z, and a box the axes
    /// never touch is an ordinary occluder for all three, however many other
    /// shapes it was merged into one mesh with (img2).
    ///
    /// The span is kept, not just the tag, because *where* a stretch of the
    /// line sits relative to it decides whether that body may hide it: only the
    /// approach, on the eye's side of the material, is drawn over the shape.
    pub(crate) through: [Vec<(f64, f64, u16)>; 3],
    /// One past the largest tag in `through`, so a lookup table indexed by tag
    /// can be sized once.
    pub(crate) tags: usize,
    /// The distance from the origin to the model's furthest vertex. What an
    /// origin axis's arms have to be longer than, or a shape standing on the
    /// origin holds the whole arm and the axis is never seen at all.
    pub(crate) reach: f64,
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

#[cfg(test)]
pub(crate) fn axis_material(items: &[Item<'_>], grid: &Grid, section: Option<Plane>) -> AxisMaterial {
    axis_material_live(items, grid, section, &Live::default())
}

/// The same, for a frame with a body being dragged (see [`Live`]).
///
/// The spans were found where the body stood when the scene was evaluated.
/// Its stretch of the scene is not drawn, so it is not in the axes' way there
/// any more; and where it is drawn instead, moved, the spans no longer say
/// where the axes run through it. The body is left out of the axis rule until
/// the drag ends and the scene is evaluated with it where it now is -- an axis
/// is drawn through it for the length of the drag.
pub(crate) fn axis_material_live(
    items: &[Item<'_>],
    grid: &Grid,
    section: Option<Plane>,
    live: &Live<'_>,
) -> AxisMaterial {
    let mut material = AxisMaterial {
        inside: [Vec::new(), Vec::new(), Vec::new()],
        through: [Vec::new(), Vec::new(), Vec::new()],
        tags: 0,
        reach: 0.0,
    };
    for (item, tag_base) in items.iter().zip(tag_bases(items)) {
        material.reach = material.reach.max(item.renderable.reach);
        // A ghost is see-through, so the axis inside it is too -- and it never
        // reaches the depth buffer either way.
        if item.style != Style::Solid || live.placed(item.renderable.id).is_some() {
            continue;
        }
        // The bodies the drag has taken out of this item.
        let gone: Vec<u16> = match live.hidden(item.renderable.id) {
            Some(range) => {
                let mut bodies: Vec<u16> =
                    item.renderable.bodies.get(range.start as usize..range.end as usize).unwrap_or(&[]).to_vec();
                bodies.sort_unstable();
                bodies.dedup();
                bodies
            }
            None => Vec::new(),
        };
        for axis in 0..3 {
            if !grid.axes[axis] {
                continue;
            }
            // Read off the renderable rather than measured again: where an axis
            // runs through a mesh is a property of the mesh, and the mesh has
            // not moved since the renderable was made.
            for &(span, body) in &item.renderable.axis_spans[axis] {
                if gone.binary_search(&body).is_ok() {
                    continue;
                }
                let tag = body_tag(body, tag_base);
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

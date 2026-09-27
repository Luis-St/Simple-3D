//! What an origin axis is drawn like where it is buried in the model.

use super::*;
use simple3d_geom::section::Plane;

/// What the axes meet in the model: where they run inside it, which solids, and the model's reach.
pub(crate) struct AxisMaterial {
    /// Per axis, the stretches inside a solid, as coordinates along that axis.
    pub(crate) inside: [Vec<(f64, f64)>; 3],
    /// Per axis, the bodies it runs through, each with its stretch and tag. Per body and axis, since a
    /// box one axis runs through still occludes the others (img2); the span decides where the approach is.
    pub(crate) through: [Vec<(f64, f64, u16)>; 3],
    /// One past the largest tag in `through`, for sizing a tag-indexed table.
    pub(crate) tags: usize,
    /// The distance to the model's furthest vertex, which the axis arms must exceed.
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
pub(crate) fn axis_material(items: &[Item<'_>], grid: &Grid, section: &[Plane]) -> AxisMaterial {
    axis_material_live(items, grid, section, &Live::default())
}

/// The same with a dragged body (see [`Live`]): its spans are stale, so it is left out of the axis
/// rule until the drag ends and the axis is drawn through it meanwhile.
pub(crate) fn axis_material_live(items: &[Item<'_>], grid: &Grid, section: &[Plane], live: &Live<'_>) -> AxisMaterial {
    let mut material = AxisMaterial {
        inside: [Vec::new(), Vec::new(), Vec::new()],
        through: [Vec::new(), Vec::new(), Vec::new()],
        tags: 0,
        reach: 0.0,
    };
    for (item, tag_base) in items.iter().zip(tag_bases(items)) {
        material.reach = material.reach.max(item.renderable.reach);
        // A ghost is see-through, so the axis inside it is too.
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
            // Read from the renderable, since the mesh has not moved since it was made.
            for &(span, body) in &item.renderable.axis_spans[axis] {
                if gone.binary_search(&body).is_ok() {
                    continue;
                }
                let tag = body_tag(body, tag_base);
                // Material the section removed no longer blocks the axis.
                let kept = match section.is_empty() {
                    true => vec![span],
                    false => trim_spans(span, axis, section),
                };
                for span in kept {
                    material.inside[axis].push(span);
                    material.through[axis].push((span.0, span.1, tag));
                    material.tags = material.tags.max(tag as usize + 1);
                }
            }
        }
    }
    material
}

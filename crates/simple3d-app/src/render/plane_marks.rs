//! The marks where the principal planes cross solids.

use super::*;
use crate::raster::Rgba;
use crate::snap::MARK_AXIS;
use crate::view::View;
use simple3d_geom::section::Plane;

/// Each plane mark's colour, by perpendicular axis: the colour of the axis it is presented as,
/// swapped for X and Y ([`crate::snap::MARK_AXIS`]).
pub(crate) fn mark_colours(palette: &Palette) -> [Rgba; 3] {
    let axes = [palette.axis_x, palette.axis_y, palette.axis_z];
    [axes[MARK_AXIS[0]], axes[MARK_AXIS[1]], axes[MARK_AXIS[2]]]
}

/// Draw where each principal plane cuts each solid's surface, showing e.g. how much is below the
/// build plate. A mark follows the switch of the axis it is drawn as, since colour is its only
/// identity (issue 75).
pub(crate) fn push_plane_marks(
    steps: &mut Vec<Step>,
    view: &View,
    items: &[Item<'_>],
    palette: &Palette,
    grid: &Grid,
    section: &[Plane],
) {
    let colours = mark_colours(palette);
    for item in items.iter().filter(|i| i.style == Style::Solid) {
        for (axis, colour) in colours.into_iter().enumerate() {
            if !grid.axes[MARK_AXIS[axis]] {
                continue;
            }
            // Camera-independent, so found once with the renderable.
            for &[a, b] in &item.renderable.plane_marks()[axis] {
                // A mark lies on a surface, so it is cut with it.
                for (a, b) in kept_line(section, a, b).iter().copied() {
                    steps.push(line_step(view, a, b, colour, MARK_BIAS, 0, true));
                }
            }
        }
    }
}

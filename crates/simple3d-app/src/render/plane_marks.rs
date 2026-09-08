//! The marks that say where the section plane stands.

use super::*;
use crate::raster::Rgba;
use crate::snap::MARK_AXIS;
use crate::view::View;
use simple3d_geom::section::Plane;

/// The colour a principal plane's mark is drawn in, indexed by the axis that
/// plane is perpendicular to: the colour of the axis the mark is *presented*
/// as, which for X and Y is the other one (see [`crate::snap::MARK_AXIS`]).
pub(crate) fn mark_colours(palette: &Palette) -> [Rgba; 3] {
    let axes = [palette.axis_x, palette.axis_y, palette.axis_z];
    [axes[MARK_AXIS[0]], axes[MARK_AXIS[1]], axes[MARK_AXIS[2]]]
}

/// Draw, on the surface of each solid, the line where a principal plane cuts
/// through it.
///
/// The ground plane crossing a shape is a real dimension -- how much of the
/// shape is below the build plate -- and until something marks it the only way
/// to read it is to orbit until the grid is edge-on. The mark is drawn on the
/// surface itself, where the plane meets it, in the colour `mark_colours` gives
/// for the axis the plane is perpendicular to.
///
/// A mark answers to the switch of the axis it is *drawn as* rather than to the
/// one its plane is perpendicular to, because its colour is the only thing there
/// is to recognise it by (issue 75).
pub(crate) fn push_plane_marks(
    steps: &mut Vec<Step>,
    view: &View,
    items: &[Item<'_>],
    palette: &Palette,
    grid: &Grid,
    section: Option<Plane>,
) {
    let colours = mark_colours(palette);
    for item in items.iter().filter(|i| i.style == Style::Solid) {
        for (axis, colour) in colours.into_iter().enumerate() {
            if !grid.axes[MARK_AXIS[axis]] {
                continue;
            }
            for tri in &item.renderable.mesh.indices {
                let world = [
                    item.renderable.mesh.positions[tri[0] as usize],
                    item.renderable.mesh.positions[tri[1] as usize],
                    item.renderable.mesh.positions[tri[2] as usize],
                ];
                let Some((a, b)) = crate::snap::plane_crossing(world, axis) else { continue };
                // A mark lies on a surface, so it goes wherever that surface
                // does.
                let Some((a, b)) = kept_line(section, a, b) else { continue };
                steps.push(line_step(view, a, b, colour, MARK_BIAS, 0, true));
            }
        }
    }
}

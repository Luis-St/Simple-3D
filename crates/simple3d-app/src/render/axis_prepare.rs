//! Working out the three axes before any of them is drawn.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_core::scene::AxisStyle;
use simple3d_geom::Vec3;

/// One piece of an origin axis, ready to draw: where it runs on screen, what
/// colour it has faded to, and which bodies may not hide it.
///
/// `seen` is indexed by body tag and is the whole of the axis rule -- a piece
/// of the line that loses the depth test is still drawn when whatever won that
/// pixel is a body the line is arriving at. It is worked out here, from the
/// model, so that both renderers answer the question the same way.
pub(crate) struct AxisStep {
    pub a: Vertex,
    pub b: Vertex,
    pub colour: Rgba,
    pub seen: std::sync::Arc<[bool]>,
}

pub(crate) fn prepare_axes(view: &View, palette: &Palette, grid: &Grid, material: &AxisMaterial) -> Vec<AxisStep> {
    let mut out = Vec::new();
    let spacing = effective_grid_spacing(view, grid.spacing);
    let radius = grid_radius(view);
    let clearance = material.reach;
    let colours = [palette.axis_x, palette.axis_y, palette.axis_z];
    let directions = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    for axis in 0..3 {
        if !grid.axes[axis] {
            continue;
        }
        // Where this axis runs through material, and so is not drawn at all...
        let inside = &material.inside[axis];
        // ...and the bodies it runs through, each with the stretch inside it, so
        // the approach to a surface is drawn over that body and the arm beyond
        // it is not.
        let through = &material.through[axis];
        // The two styles are two different things, and each is drawn as what it
        // is. Along the grid, an axis *is* a grid line: it runs the width of the
        // ground, travels with it, and fades out with it at the edge -- so X and
        // Y are the coloured lines through zero, the way every other 3D
        // application draws them. Pinned at the origin, it is a cross: a
        // bounded mark of a dozen grid squares that stays at zero while the
        // camera pans away from it, drawn at full strength so it reads as an
        // object rather than as ground.
        let (centre, length, reach) = match grid.style {
            AxisStyle::Grid => {
                let centre = match axis {
                    // Snapped to the grid's own spacing, so the axis lies along
                    // a grid line rather than between two of them.
                    0 => Vec3::new((view.camera.target.x / spacing).round() * spacing, 0.0, 0.0),
                    1 => Vec3::new(0.0, (view.camera.target.y / spacing).round() * spacing, 0.0),
                    // Z has no grid line to be: the grid is the ground.
                    _ => Vec3::ZERO,
                };
                (centre, radius, radius)
            }
            // `reach` past the length, so the fade only softens the last part of
            // each arm instead of consuming the whole of it.
            AxisStyle::Origin => {
                // Bounded, and always well inside the frame, so it reads as a
                // cross at the origin rather than as another pair of grid lines
                // however far the camera is pulled back. Measured against the
                // frame rather than the ground's reach, which a tilt stretches.
                let frame_reach = frame_reach(view);
                let length = (spacing * 12.0).min(frame_reach * 0.61);
                // ...but never so short that the model swallows the whole arm.
                // The arms are measured from the origin, which is inside a shape
                // standing on it, so an arm that ends inside that shape is an
                // axis with nothing left to draw -- which is what zooming in on
                // a box at the origin gave: a mark on the face and no line
                // arriving at it.
                let clear = (clearance * 1.6 + 30.0 / view.pixels_per_mm().max(1e-9)).min(frame_reach * 2.0);
                let length = length.max(clear);
                (Vec3::ZERO, length, length * 2.0)
            }
        };
        // Both halves fade outward from the centre, for the same reason the
        // grid does: an axis that ends abruptly reads as an object.
        for sign in [-1.0, 1.0] {
            push_axis_line(
                &mut out,
                view,
                centre,
                centre + directions[axis] * (length * sign),
                reach,
                colours[axis],
                axis,
                inside,
                through,
                material.tags,
            );
        }
    }
    out
}

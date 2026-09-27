//! Preparing the three axes before any is drawn.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_core::scene::AxisStyle;
use simple3d_geom::Vec3;

/// One axis piece ready to draw: its screen line, faded colour, and which bodies may not hide it.
/// `seen` (by body tag) is the axis rule, computed here so both renderers agree.
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
        // Where this axis runs through material and is not drawn...
        let inside = &material.inside[axis];
        // ...and the bodies it runs through with their stretches, so the approach is drawn over the body
        // and the arm beyond is not.
        let through = &material.through[axis];
        let (centre, length, reach) = axis_layout(view, grid, spacing, radius, clearance, axis);
        // Both halves fade outward, since an abruptly ending axis reads as an object.
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

/// Where one axis is drawn: its centre, arm length and fade distance. Shared by both engines.
pub(crate) fn axis_layout(
    view: &View,
    grid: &Grid,
    spacing: f64,
    radius: f64,
    clearance: f64,
    axis: usize,
) -> (Vec3, f64, f64) {
    // Grid style: the axis is a grid line through zero, as wide as the ground and fading with it.
    // Origin style: a bounded, full-strength cross fixed at zero.
    match grid.style {
        AxisStyle::Grid => {
            let centre = match axis {
                // Snapped to the grid spacing, so the axis lies on a grid line.
                0 => Vec3::new((view.camera().target.x / spacing).round() * spacing, 0.0, 0.0),
                1 => Vec3::new(0.0, (view.camera().target.y / spacing).round() * spacing, 0.0),
                // Z has no grid line: the grid is the ground.
                _ => Vec3::ZERO,
            };
            (centre, radius, radius)
        }
        // `reach` past the length, so the fade only softens each arm's end.
        AxisStyle::Origin => {
            // Bounded well inside the frame so it reads as a cross, measured against the frame, not the
            // tilt-stretched ground reach...
            let frame_reach = frame_reach(view);
            let length = (spacing * 12.0).min(frame_reach * 0.61);
            // ...but never so short that a shape at the origin swallows the whole arm.
            let clear = (clearance * 1.6 + 30.0 / view.pixels_per_mm().max(1e-9)).min(frame_reach * 2.0);
            let length = length.max(clear);
            (Vec3::ZERO, length, length * 2.0)
        }
    }
}

//! The axis drivers shared by several shapes, where a handle does not map
//! straight onto one parameter.

use super::*;
use std::f64::consts::PI;

/// Inner diameter of a tube/ring, honouring the wall-thickness / inner-diameter
/// choice. Clamped so a wall thicker than the radius degenerates to a solid
/// rather than producing inverted geometry.
pub(crate) fn tube_inner(p: &Params) -> f64 {
    let outer = p.num("outer_diameter");
    let inner = if p.int("wall_mode") == 0 { outer - 2.0 * p.num("wall_thickness") } else { p.num("inner_diameter") };
    inner.clamp(0.0, outer)
}

/// Whether a revolved shape goes all the way round. Short of that it is a pie
/// slice, and its width across X and Y is no longer its diameter -- which is
/// why the resize handles on those axes withdraw.
pub(crate) fn fully_swept(p: &Params) -> bool {
    p.num("sweep") >= 360.0
}

/// The X and Y resize handles of a revolved shape, offered only while it is a
/// full turn and its diameter really is its width.
pub(crate) fn round_axes(p: &Params, x: &'static str, y: &'static str) -> [Option<AxisDriver>; 2] {
    if fully_swept(p) {
        [Some(AxisDriver::direct(x)), Some(AxisDriver::direct(y))]
    } else {
        [None, None]
    }
}

/// Extent-to-diameter factor for a regular n-gon generated with a vertex at
/// angle 0: X spans the across-corners distance, so an across-flats diameter
/// has to be divided by cos(pi/n) to get it.
pub(crate) fn polygon_x_factor(p: &Params) -> f64 {
    let sides = p.int("sides").max(3);
    if p.int("measure") == 1 {
        1.0 / (PI / sides as f64).cos()
    } else {
        1.0
    }
}

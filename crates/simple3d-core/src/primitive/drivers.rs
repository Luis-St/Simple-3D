//! Axis drivers shared by several shapes, where a handle is not one parameter.

use super::*;
use std::f64::consts::PI;

/// Inner diameter of a tube or ring, per the wall/inner choice. Clamped so an over-thick wall
/// degenerates to a solid instead of inverting, and an inner diameter past the outer one leaves a
/// hundredth of it as wall: a wall of nothing has no volume, which rounding can make negative.
pub(crate) fn tube_inner(p: &Params) -> f64 {
    let outer = p.num("outer_diameter");
    let inner = if p.int("wall_mode") == 0 { outer - 2.0 * p.num("wall_thickness") } else { p.num("inner_diameter") };
    inner.clamp(0.0, outer * 0.99)
}

/// Whether a revolved shape is a full turn; a pie slice's width is not its diameter.
pub(crate) fn fully_swept(p: &Params) -> bool {
    p.num("sweep") >= 360.0
}

/// X and Y resize handles of a revolved shape, only while it is a full turn.
pub(crate) fn round_axes(p: &Params, x: &'static str, y: &'static str) -> [Option<AxisDriver>; 2] {
    if fully_swept(p) {
        [Some(AxisDriver::direct(x)), Some(AxisDriver::direct(y))]
    } else {
        [None, None]
    }
}

/// Extent-to-diameter factor for an n-gon with a vertex at angle 0: X spans from that corner to the
/// opposite corner, or to the opposite flat when n is odd, and an across-flats diameter is the
/// circumradius times 2cos(pi/n).
pub(crate) fn polygon_x_factor(p: &Params) -> f64 {
    let sides = p.int("sides").max(3);
    let flat = (PI / sides as f64).cos();
    let span = if sides.is_multiple_of(2) { 1.0 } else { (1.0 + flat) / 2.0 };
    if p.int("measure") == 1 {
        span / flat
    } else {
        span
    }
}

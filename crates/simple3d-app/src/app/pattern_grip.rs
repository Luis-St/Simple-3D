//! A handle on a pattern, and the number it writes back.

use simple3d_geom::Vec3;

/// One of a pattern's lay-out grips (issue 67) in world space, ready to project, hit-test and drag.
#[derive(Clone, Copy, Debug)]
pub struct PatternGrip {
    /// The grip's name, which identifies it across a drag, where indices could shift.
    pub label: &'static str,
    /// Where the grip sits, in world space.
    pub at: Vec3,
    /// The world line it slides along: a point on it, and a unit direction.
    pub from: Vec3,
    pub dir: Vec3,
    /// World millimetres per pattern-frame millimetre along the line, so scaled patterns read back
    /// their own numbers.
    pub scale: f64,
    /// For a span grip: the world turn axis, the zero-degree direction, and the grip's radius; `None`
    /// for a sliding grip.
    pub turn: Option<(Vec3, Vec3, f64)>,
}

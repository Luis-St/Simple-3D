//! A handle on a pattern, and the number it writes back.

use simple3d_geom::Vec3;

/// One of a pattern's lay-out grips (issue 67), placed in the world.
///
/// [`simple3d_core::pattern::grips`] works in the pattern's own frame, which is
/// where the copies are laid out; this is the same grip after the node's
/// transform, ready to be projected, hit-tested and dragged.
#[derive(Clone, Copy, Debug)]
pub struct PatternGrip {
    /// The grip's own name, which is what a drag holds on to across frames: an
    /// index would shift under the drag itself, since adding a copy can add a
    /// grip.
    pub label: &'static str,
    /// Where the grip sits, in world space.
    pub at: Vec3,
    /// The world line it slides along: a point on it, and a unit direction.
    pub from: Vec3,
    pub dir: Vec3,
    /// How many world millimetres one millimetre of the pattern's own frame
    /// covers along that line, so a scaled pattern still reads back the numbers
    /// its property editor shows.
    pub scale: f64,
    /// For a span grip: the world axis it turns about, the world direction zero
    /// degrees points in, and how far out the grip rides. `None` for a grip that
    /// slides along a line.
    pub turn: Option<(Vec3, Vec3, f64)>,
}

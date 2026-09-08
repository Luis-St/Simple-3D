//! What one frame was asked for.

use super::*;
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

pub struct Grid {
    pub visible: bool,
    /// Spacing in millimetres.
    pub spacing: f64,
    /// Which of the three origin axes to draw, X, Y, Z. The axes are laid out on
    /// the grid's spacing, which is why they are described here with it.
    pub axes: [bool; 3],
    pub style: AxisStyle,
    /// Whether to mark, on a solid's own surface, where a principal plane cuts
    /// through it. Each plane is named by the axis it is perpendicular to and
    /// follows that axis's switch, so the ground plane's mark is the Z one.
    pub plane_marks: bool,
}

/// One thing to draw, in world space.
pub struct Item<'a> {
    pub renderable: &'a Renderable,
    pub style: Style,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The evaluated scene.
    Solid,
    /// The selected node's own geometry, drawn over the top so it is visible
    /// even where a boolean consumed it.
    Selected,
    /// A hidden node, so a subtracted tool body can be seen while it is being
    /// positioned.
    Ghost,
    /// Outlined like a selection *and* filled with a glow that whatever is in
    /// front of it does not hide: a body that has to be found inside another
    /// one, which is what a piece of a split usually is (issue 82). An outline
    /// alone cannot say where a piece is when the piece is buried -- there is
    /// nothing of it on screen to outline.
    Glow,
}

pub struct Request<'a> {
    pub view: View,
    pub size: [usize; 2],
    pub mode: DisplayMode,
    pub palette: Palette,
    pub grid: Grid,
    pub items: Vec<Item<'a>>,
    /// A tool's preview, in world space: closed loops drawn over the model in
    /// the accent colour once everything else is down (issue 82).
    ///
    /// It is drawn here rather than with the 2D painter over the finished
    /// picture so that it meets the depth buffer: a cut on the far side of the
    /// shape is behind it, and a grid that shows through the solid it lies on
    /// reads as floating in front of it. It writes no depth of its own -- one
    /// loop must not hide the next where they cross -- and it is biased towards
    /// the eye, because the loops at the ends of a run lie exactly on the
    /// surface they are drawn on and would otherwise lose the tie to it.
    pub preview: Vec<Vec<Vec3>>,
    /// The plane the model is cut with, or `None` for the whole of it
    /// (issue 71). Everything drawn from the model goes through it -- faces,
    /// edges, outlines, ghosts, marks, the stretches of an axis that run
    /// through material -- and the opening it leaves is closed with a cap, so
    /// that a wall reads as a wall rather than as a shell seen from inside.
    pub section: Option<Plane>,
}

/// How many rows a band must have before splitting the frame again is worth
/// the thread it costs. Below this the whole frame goes to one band: a small
/// viewport rasterizes in well under a millisecond, and spawning eight threads
/// to share that out costs more than it saves.
pub(crate) const MIN_BAND_ROWS: usize = 96;

/// How many bands to cut the frame into: one per core, but never so many that
/// they stop being worth starting.
pub(crate) fn band_count(height: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    (height / MIN_BAND_ROWS).clamp(1, cores)
}

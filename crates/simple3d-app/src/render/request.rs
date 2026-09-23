//! What one frame was asked for.

use super::*;
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
use simple3d_core::xform::Xform;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;
use std::ops::Range;

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

/// A body being dragged, drawn where the drag has got to rather than where
/// the last evaluation put it.
///
/// Moving, turning or scaling a body changes none of its geometry, only where
/// it stands, and that is a transform the card can apply for nothing. So while
/// a body is dragged the viewport does not wait for the scene to be evaluated
/// again: the body's stretch of the scene is left out, and the body's own
/// renderable is drawn in its place, moved by how far it has gone. Each is
/// named by its renderable's id.
#[derive(Clone, Default)]
pub struct Live<'a> {
    /// Welded vertices of a renderable to leave out, with every triangle and
    /// edge that uses them: the dragged body's part of the scene.
    pub hidden: Vec<(u64, Range<u32>)>,
    /// Renderables to draw moved: each world position is put through the
    /// transform first.
    pub placed: Vec<(u64, Xform)>,
    /// A boolean drawn per pixel from the shapes that go into it, where the
    /// part of the scene it stands for has been left out (`hidden`): what a
    /// drag of one of its operands shows. See `App::live_csg`.
    pub csg: Option<CsgPreview<'a>>,
    /// The shapes such a boolean would be drawn from if the selection were
    /// dragged, to be put on the card ahead of it and kept there without
    /// being drawn, so the drag's first frame has nothing to upload. See
    /// `App::csg_ready`.
    pub ready: Vec<&'a Renderable>,
}

/// A boolean the GPU works out per pixel -- see [`Live::csg`].
#[derive(Clone)]
pub struct CsgPreview<'a> {
    /// The shapes, in world space, each moved or not.
    pub leaves: Vec<(&'a Renderable, Option<Xform>)>,
    /// The expression over them, in postfix: a leaf by its index, or one of
    /// `CSG_UNION`, `CSG_DIFFERENCE`, `CSG_INTERSECTION` applied to the two
    /// values before it.
    pub program: Vec<i32>,
    /// The body tag the result is drawn with, so an origin axis treats it as
    /// the body it stands for.
    pub tag: u16,
}

impl Live<'_> {
    pub fn hidden(&self, id: u64) -> Option<&Range<u32>> {
        self.hidden.iter().find(|(of, _)| *of == id).map(|(_, range)| range)
    }

    pub fn placed(&self, id: u64) -> Option<&Xform> {
        self.placed.iter().find(|(of, _)| *of == id).map(|(_, xform)| xform)
    }
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
    /// What a drag has moved since the renderables were made, drawn where it
    /// has got to. Only the GPU renderer is ever handed any: see [`Live`].
    pub live: Live<'a>,
    /// The plane the model is cut with, or `None` for the whole of it
    /// (issue 71). Everything drawn from the model goes through it -- faces,
    /// edges, outlines, ghosts, marks, the stretches of an axis that run
    /// through material -- and the opening it leaves is closed with a cap, so
    /// that a wall reads as a wall rather than as a shell seen from inside.
    pub section: Option<Plane>,
}

/// How many rows a band must have, on average, before splitting the frame
/// again is worth the thread it costs. Below this the whole frame goes to one
/// band: a small viewport rasterizes in well under a millisecond, and spawning
/// eight threads to share that out costs more than it saves.
///
/// It used to be 96, which on a frame a thousand pixels tall allowed ten bands
/// whatever the machine had: a dense mesh costs per triangle rather than per
/// pixel, and ten threads were all it ever got. The bands are cut by work now
/// (`balanced_ranges`), so a band this short across the model is as busy as a
/// tall one across the sky.
pub(crate) const MIN_BAND_ROWS: usize = 24;

/// How many bands to cut the frame into: one per core, but never so many that
/// they stop being worth starting.
pub(crate) fn band_count(height: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    (height / MIN_BAND_ROWS).clamp(1, cores)
}

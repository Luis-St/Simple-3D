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
    /// Which origin axes to draw (X, Y, Z); laid out on the grid spacing, hence here.
    pub axes: [bool; 3],
    pub style: AxisStyle,
    /// Whether to mark where each principal plane cuts solids; each follows its axis's switch.
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
    /// The selected node's own geometry, drawn on top so it shows even where a boolean consumed it.
    Selected,
    /// A hidden node, so a subtracted tool body can be seen while positioning it.
    Ghost,
    /// Outlined and filled with a glow nothing in front hides: a buried body such as a split piece
    /// (issue 82).
    Glow,
}

/// A body being dragged, drawn where the drag has taken it. Only its placement changes, so its
/// scene range is hidden and its own renderable drawn moved instead of re-evaluating. Keyed by
/// renderable id.
#[derive(Clone, Default)]
pub struct Live<'a> {
    /// Welded vertex ranges to hide, with their triangles and edges: the dragged body's part.
    pub hidden: Vec<(u64, Range<u32>)>,
    /// Renderables to draw moved by the transform.
    pub placed: Vec<(u64, Xform)>,
    /// A per-pixel boolean standing in for the hidden part, when a drag moves an operand
    /// (`App::live_csg`).
    pub csg: Option<CsgPreview<'a>>,
    /// Shapes to upload ahead of a possible drag without drawing them (`App::csg_ready`).
    pub ready: Vec<&'a Renderable>,
}

/// A boolean the GPU computes per pixel (see [`Live::csg`]).
#[derive(Clone)]
pub struct CsgPreview<'a> {
    /// The shapes in world space, each optionally moved.
    pub leaves: Vec<(&'a Renderable, Option<Xform>)>,
    /// The postfix expression: a leaf index, or `CSG_UNION`, `CSG_DIFFERENCE` or `CSG_INTERSECTION`
    /// on the two values before it.
    pub program: Vec<i32>,
    /// The body tag the result is drawn with, so an origin axis treats it as that body.
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
    /// A tool's preview loops in world space, drawn in the accent over the model (issue 82). Drawn
    /// by the renderer so they are depth-tested; they write no depth and are biased towards the eye,
    /// since loops on the surface would otherwise lose the tie.
    pub preview: Vec<Vec<Vec3>>,
    /// Bodies drawn translucent where a tool would put them, as a template of the result: the align
    /// tool's moved objects and the copies it would make (issue 70). Each is a renderable and the
    /// world transform carrying it there.
    pub templates: Vec<(&'a Renderable, Xform)>,
    /// What a drag has moved since the renderables were made; GPU renderer only (see [`Live`]).
    pub live: Live<'a>,
    /// The section planes (issue 71). Everything drawn from the model is cut by them, and the opening
    /// is capped so walls read as solid.
    pub section: Vec<Plane>,
}

/// The fewest rows per band before splitting further is worth a thread. Bands are cut by work
/// (`balanced_ranges`), so this can be low.
pub(crate) const MIN_BAND_ROWS: usize = 24;

/// How many bands to split the frame into: one per core, but not too small to be worth it.
pub(crate) fn band_count(height: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    (height / MIN_BAND_ROWS).clamp(1, cores)
}

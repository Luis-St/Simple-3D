//! The sketch editor (issue 74): drawing a 2D profile on the ground, a workplane or a face,
//! constraining it, picking the regions to keep and turning them into a solid.
//!
//! The editor is a mode of its own. It starts by picking the plane -- a click on one of the orange
//! origin planes sketches on it, on a face of the model on that face, anywhere else on the plane being
//! worked on -- then turns the camera
//! to look straight at it and shows its grid. The viewport's clicks then draw, select and drag in the
//! sketch, and the window holds the drawing tools, the constraints, the list of what is constrained
//! and the feature settings. Nothing enters the document until Finish, which adds the sketch node,
//! or puts the changed sketch back on the node being edited, as one undo step; the editor keeps its
//! own undo while it is open.

mod constrain;
mod drawing;
mod hit;
mod open;
mod origin;
mod overlay;
pub(crate) use overlay::*;
mod glyphs;
mod pointer;
pub(crate) use pointer::*;
mod window;
pub(crate) use window::*;
#[cfg(test)]
mod tests;
mod window_feature;
mod window_list;

use simple3d_core::primitive::Params;
use simple3d_core::scene::{NodeId, Placing};
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;
use simple3d_sketch::solver::Analysis;
use simple3d_sketch::{ConstraintId, CurveId, PointId, Sketch};

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "sketch-tool";
/// The window width: two columns of tool buttons and the constraint list beside its values.
const WIDTH: f32 = 340.0;
/// How many steps the editor's own undo keeps.
const UNDO_DEPTH: usize = 200;

/// What a click in the viewport does while the sketch is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Draw {
    Select,
    Line,
    Rectangle,
    Circle,
    Arc,
    /// A spline through the clicked points.
    Spline,
    /// A spline pulled toward the clicked points.
    Bspline,
    Point,
    /// Click regions to make the solid from.
    Regions,
    /// Click the line a revolve turns about.
    Axis,
    /// Click a face of the model to copy its edges into the sketch.
    CopyEdges,
}

impl Draw {
    pub(crate) const DRAWING: [Draw; 8] =
        [Draw::Select, Draw::Line, Draw::Rectangle, Draw::Circle, Draw::Arc, Draw::Spline, Draw::Bspline, Draw::Point];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Draw::Select => "Select",
            Draw::Line => "Line",
            Draw::Rectangle => "Rectangle",
            Draw::Circle => "Circle",
            Draw::Arc => "Arc",
            Draw::Spline => "Spline",
            Draw::Bspline => "Control spline",
            Draw::Point => "Point",
            Draw::Regions => "Pick regions",
            Draw::Axis => "Revolve axis",
            Draw::CopyEdges => "Copy face edges",
        }
    }

    /// What to do next with this tool, for the window's hint line.
    pub(crate) fn hint(self) -> &'static str {
        match self {
            Draw::Select => "Click points, curves or constraints to select them, Shift adds; drag to move.",
            Draw::Line => "Click to put lines down one after another; right-click or Esc ends the chain.",
            Draw::Rectangle => "Click two opposite corners.",
            Draw::Circle => "Click the centre, then a point on the circle.",
            Draw::Arc => "Click the centre, the start, then the end; the arc runs counter-clockwise.",
            Draw::Spline => "Click the points it passes through; right-click ends it, a click on the first closes it.",
            Draw::Bspline => "Click the points that pull on it; right-click ends it, a click on the first closes it.",
            Draw::Point => "Click to put a point down.",
            Draw::Regions => "Click the closed areas to make the solid from; click again to drop one.",
            Draw::Axis => "Click the line to revolve about.",
            Draw::CopyEdges => "Click a face of the model to copy its edges onto the sketch.",
        }
    }
}

/// Something in the sketch that can be selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Pick {
    Point(PointId),
    Curve(CurveId),
    Constraint(ConstraintId),
}

/// A click of a drawing gesture: where, and the sketch point it landed on, if any.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Click {
    pub at: [f64; 2],
    pub point: Option<PointId>,
}

/// Points being dragged, each with where it was when the drag began.
pub(crate) struct Dragging {
    pub start: [f64; 2],
    pub points: Vec<(PointId, [f64; 2])>,
    pub before: Sketch,
}

/// What the editor is working on while it is open.
pub struct SketchTool {
    pub sketch: Sketch,
    /// The feature: extrude or revolve, and how far.
    pub params: Params,
    /// The sketch node being changed, or `None` for a new sketch.
    pub editing: Option<NodeId>,
    /// The sketch plane in the world, or `None` until it is picked.
    pub frame: Option<Xform>,
    /// The object whose face it was started on; the solid is added beside it or cut out of it.
    pub owner: Option<NodeId>,
    pub placing: Placing,
    pub(crate) draw: Draw,
    pub(crate) clicks: Vec<Click>,
    pub(crate) selection: Vec<Pick>,
    pub(crate) hover: Option<Pick>,
    pub(crate) drag: Option<Dragging>,
    pub(crate) undo: Vec<Sketch>,
    pub(crate) redo: Vec<Sketch>,
    pub(crate) analysis: Analysis,
    /// Constraints the last refused one would have fought, marked until the next change.
    pub(crate) conflicts: Vec<ConstraintId>,
    /// Whether copied face edges come in as helper geometry rather than as part of the profile.
    pub(crate) copy_as_helper: bool,
    /// The display unit, for dimension labels; kept up to date while the editor is drawn.
    pub(crate) unit: simple3d_core::unit::Unit,
}

impl SketchTool {
    /// A new, empty sketch waiting for its plane.
    pub fn new() -> SketchTool {
        SketchTool::with(Sketch::default(), simple3d_core::scene::sketch_params(&Params::new()), None, None)
    }

    /// The editor on an existing sketch node, its plane already known.
    pub fn editing(id: NodeId, sketch: Sketch, params: Params, frame: Xform) -> SketchTool {
        SketchTool::with(sketch, params, Some(id), Some(frame))
    }

    fn with(sketch: Sketch, params: Params, editing: Option<NodeId>, frame: Option<Xform>) -> SketchTool {
        let analysis = sketch.analyse();
        SketchTool {
            sketch,
            params,
            editing,
            frame,
            owner: None,
            placing: Placing::Add,
            draw: Draw::Line,
            clicks: Vec::new(),
            selection: Vec::new(),
            hover: None,
            drag: None,
            undo: Vec::new(),
            redo: Vec::new(),
            analysis,
            conflicts: Vec::new(),
            copy_as_helper: true,
            unit: simple3d_core::unit::Unit::default(),
        }
    }

    /// The sketch point at `p` in the world.
    pub fn world(&self, p: [f64; 2]) -> Vec3 {
        self.frame.map_or(Vec3::new(p[0], p[1], 0.0), |frame| frame.point(Vec3::new(p[0], p[1], 0.0)))
    }

    /// World point `p` in the sketch's plane, dropped onto it along its normal.
    pub fn local(&self, p: Vec3) -> [f64; 2] {
        let q = self.frame.map_or(p, |frame| frame.inverse().point(p));
        [q.x, q.y]
    }

    /// Change the sketch through `change`, keeping the old one for undo and solving after. Returns
    /// what `change` did.
    pub(crate) fn change<T>(&mut self, change: impl FnOnce(&mut Sketch) -> T) -> T {
        let before = self.sketch.clone();
        let out = change(&mut self.sketch);
        if self.sketch != before {
            self.remember(before);
            self.sketch.solve();
            self.refresh();
        }
        out
    }

    /// Keep `before` as the step undo goes back to.
    pub(crate) fn remember(&mut self, before: Sketch) {
        self.undo.push(before);
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Re-read what the constraints leave free, and drop what no longer exists from the selection.
    pub(crate) fn refresh(&mut self) {
        self.analysis = self.sketch.analyse();
        self.conflicts.clear();
        let sketch = &self.sketch;
        self.selection.retain(|pick| match *pick {
            Pick::Point(id) => sketch.point(id).is_some(),
            Pick::Curve(id) => sketch.curve(id).is_some(),
            Pick::Constraint(id) => sketch.constraint(id).is_some(),
        });
        self.hover = None;
    }

    pub(crate) fn undo_step(&mut self) {
        if let Some(before) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.sketch, before));
            self.clicks.clear();
            self.refresh();
        }
    }

    pub(crate) fn redo_step(&mut self) {
        if let Some(after) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.sketch, after));
            self.clicks.clear();
            self.refresh();
        }
    }

    /// Pick another tool, dropping a gesture half done.
    pub(crate) fn set_draw(&mut self, draw: Draw) {
        self.draw = draw;
        self.clicks.clear();
    }
}

impl Default for SketchTool {
    fn default() -> Self {
        SketchTool::new()
    }
}

//! The sketch editor (issue 74): drawing a 2D profile on the ground, a workplane or a face,
//! constraining it, picking the regions to keep and turning them into a solid.
//!
//! The editor is a mode of its own. It starts by picking the plane -- a click on one of the orange
//! origin planes sketches on it, on a face of the model on that face, anywhere else on the plane being
//! worked on -- then turns the camera
//! to look straight at it and shows its grid. The viewport's clicks then draw, select and drag in the
//! sketch, and the editor's panel holds the drawing and modifying tools, the constraints, the list of
//! what is constrained and the feature settings; while the docks are shown it takes the right dock's place, else it floats
//! over the viewport. Clicking a curve, point or constraint opens a small popup beside it to change
//! that one thing. Nothing enters the document until Finish, which adds the sketch node,
//! or puts the changed sketch back on the node being edited, as one undo step; the editor keeps its
//! own undo while it is open.

mod area;
mod constrain;
mod constrain_pick;
mod constrain_text;
mod dimension;
mod dimension_draw;
mod dock;
pub(crate) use dock::*;
mod drawing;
mod drawing_more;
mod drawing_tangent;
mod gesture;
mod history;
mod hit;
mod infer;
mod infer_draw;
mod inspect;
mod inspect_constraint;
mod inspect_ellipse;
mod inspect_fields;
mod inspect_widgets;
mod keys;
mod label_drag;
mod modify;
mod modify_copy;
mod modify_ends;
mod modify_offset;
mod picks;
mod picks_more;
mod readout;
mod shapes;
mod tools;
pub(crate) use tools::{Draw, Family};
mod settings;
mod tools_text;
mod view;
pub(crate) use view::held_back;
mod open;
mod origin;
mod overlay;
pub(crate) use overlay::*;
mod glyphs;
mod plane_pick;
pub(crate) use plane_pick::*;
mod pointer;
pub(crate) use pointer::*;
mod window;
pub(crate) use window::*;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_constrain;
#[cfg(test)]
mod tests_grid;
#[cfg(test)]
mod tests_modify;
#[cfg(test)]
mod tests_tools;
#[cfg(test)]
mod tests_variants;
#[cfg(test)]
mod tests_view;
mod window_feature;
mod window_list;
mod window_tools;

use simple3d_core::primitive::Params;
use simple3d_core::scene::{NodeId, Placing};
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;
use simple3d_sketch::solver::Analysis;
use simple3d_sketch::{ConstraintId, CurveId, PointId, Sketch};

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "sketch-tool";
/// The floating window's width, when the docks are hidden: the constraint list beside its values.
const WIDTH: f32 = 340.0;
/// How many steps the editor's own undo keeps.
const UNDO_DEPTH: usize = 200;

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
    /// The area selection being drawn, if any.
    pub(crate) area: Option<area::Area>,
    pub(crate) undo: Vec<Sketch>,
    pub(crate) redo: Vec<Sketch>,
    pub(crate) analysis: Analysis,
    /// Constraints the last refused one would have fought, marked until the next change.
    pub(crate) conflicts: Vec<ConstraintId>,
    /// The display unit, for dimension labels; kept up to date while the editor is drawn.
    pub(crate) unit: simple3d_core::unit::Unit,
    /// What the popup beside the pointer changes, if one is open.
    pub(crate) inspect: Option<Pick>,
    /// Where on screen the popup was asked for, until it has been placed there.
    pub(crate) inspect_at: Option<egui::Pos2>,
    /// What the tools are set to.
    pub(crate) settings: settings::Settings,
    /// The popup field the last undo step came from, so a drag on one field is one step.
    pub(crate) field_step: Option<String>,
    /// The 2D lock: the view stays straight on the sketch, with orbiting and view changes held back.
    pub(crate) flat: bool,
    /// Whether Ctrl is held, drawing freely without inferring constraints; kept up to date while
    /// the pointer is over the viewport.
    pub(crate) free: bool,
    /// Constraints inferred by the gesture's clicks, waiting for the click that adds what they hold.
    pub(crate) inferred: Vec<infer::Infer>,
    /// The constraint the panel's list is pointing at, lit up in the sketch.
    pub(crate) list_hover: Option<ConstraintId>,
    /// The dimension label being dragged, with the sketch as the drag began.
    pub(crate) label_drag: Option<(ConstraintId, Sketch)>,
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
            area: None,
            undo: Vec::new(),
            redo: Vec::new(),
            analysis,
            conflicts: Vec::new(),
            unit: simple3d_core::unit::Unit::default(),
            inspect: None,
            inspect_at: None,
            settings: settings::Settings::default(),
            field_step: None,
            flat: true,
            free: false,
            inferred: Vec::new(),
            list_hover: None,
            label_drag: None,
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
        self.field_step = None;
    }

    /// Re-read what the constraints leave free, and drop what no longer exists from the selection.
    pub(crate) fn refresh(&mut self) {
        self.analysis = self.sketch.analyse();
        self.conflicts.clear();
        let sketch = &self.sketch;
        let exists = |pick: &Pick| match *pick {
            Pick::Point(id) => sketch.point(id).is_some(),
            Pick::Curve(id) => sketch.curve(id).is_some(),
            Pick::Constraint(id) => sketch.constraint(id).is_some(),
        };
        self.selection.retain(exists);
        if !self.inspect.as_ref().is_some_and(exists) {
            self.inspect = None;
        }
        self.hover = None;
    }

    /// Pick another tool, dropping a gesture half done.
    pub(crate) fn set_draw(&mut self, draw: Draw) {
        self.draw = draw;
        self.settings.remember(draw);
        self.clicks.clear();
        self.inferred.clear();
    }

    /// Open the popup on `pick`, beside screen point `at`.
    pub(crate) fn open_inspect(&mut self, pick: Pick, at: egui::Pos2) {
        if self.inspect != Some(pick) {
            self.field_step = None;
        }
        self.inspect = Some(pick);
        self.inspect_at = Some(at);
    }
}

impl Default for SketchTool {
    fn default() -> Self {
        SketchTool::new()
    }
}

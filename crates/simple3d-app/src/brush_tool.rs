//! The brush (issue 127): shaping a mesh body's surface by brushing over it, as a terrain editor
//! shapes its ground. Raise, lower, smooth, flatten or roughen with noise under a circle that
//! follows the surface; a press and drag lays dabs along the path, a quarter radius apart, and a
//! press held still keeps dabbing where it is.
//!
//! A stroke edits the working copy (`crate::sculpt`) and reaches the document only when it ends, as
//! one undo step; the panels and the status bar show the mesh as committed until then. Ctrl inverts
//! raise and lower, Shift smooths while held; the brush's size and strength are on keys
//! (`Command::BrushSmaller` and the others) and on the wheel with Ctrl, and Ctrl+Shift.

mod apply;
mod draw;
pub(crate) use draw::draw;
mod interact;
pub(crate) use interact::*;
mod window;
pub(crate) use window::show;

use crate::app::App;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// How much one key press changes the size or strength: a quarter, so a few presses go a long way.
pub(crate) const STEP: f64 = 1.25;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "brush-tool";

/// What the tool holds between frames.
#[derive(Default)]
pub(crate) struct BrushTool {
    /// Where the circle is: the surface point under the pointer and its normal.
    pub hover: Option<(Vec3, Vec3)>,
    /// The mesh body under the pointer, which a press selects and brushes: the brush needs no
    /// selecting first.
    pub over: Option<NodeId>,
    /// The stroke being dragged.
    pub stroke: Option<Dragging>,
}

/// A stroke as the pointer drags it.
pub(crate) struct Dragging {
    /// Where the pointer was last frame, in screen points: the path is followed from there.
    pub pointer: egui::Pos2,
    /// Where the last dab landed, so the next comes a spacing further on.
    pub last: Vec3,
    /// Seconds the pointer has stayed put since the last dab: held still, the stroke dabs on.
    pub held: f32,
}

/// The node a stroke works on: the one selected node, when it is a shown mesh body. A press on a
/// mesh body selects it first, so this is the body pressed on.
pub(crate) fn target(app: &App) -> Option<NodeId> {
    let selected = app.top_level_selection();
    let &[id] = selected.as_slice() else { return None };
    app.scene.get(id).filter(|node| node.mesh().is_some() && app.scene.is_shown(id)).map(|_| id)
}

/// What the status bar says when the brush is picked.
pub(crate) fn hint(app: &App) -> &'static str {
    match app.brush.over.or_else(|| target(app)) {
        Some(_) => "Brush: drag over the mesh to shape it. Ctrl inverts, Shift smooths, Ctrl+wheel sizes the brush",
        None => "Brush: drag over a mesh body to brush it (convert a shape to a mesh first)",
    }
}

impl App {
    /// Scale the brush's radius and strength by these factors, from a key or the wheel.
    pub(crate) fn resize_brush(&mut self, radius: f64, strength: f64) {
        let brush = &mut self.settings.last_brush;
        let (lo, hi) = simple3d_geom::brush::Brush::RADIUS;
        brush.radius = (brush.radius * radius).clamp(lo, hi);
        let (lo, hi) = simple3d_geom::brush::Brush::STRENGTH;
        brush.strength = (brush.strength * strength).clamp(lo, hi);
        let (radius, strength) = (brush.radius, brush.strength);
        let unit = self.unit();
        self.status = crate::app::Status::Info(format!(
            "Brush radius {} {}, strength {:.0}%",
            simple3d_core::unit::format_length(radius, unit),
            unit.suffix(),
            strength * 100.0
        ));
        self.persist();
    }
}

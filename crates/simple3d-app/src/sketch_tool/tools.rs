//! The editor's tools: what a click in the viewport does, the families the panel groups them into --
//! one tile per family, its variants in a row under the tiles -- and the key that picks each family
//! while sketching. Their names, hints and glyphs are in `tools_text`.

use super::constrain::Rule;

/// What a click in the viewport does while the sketch is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Draw {
    Select,
    Line,
    /// A line from its middle out to one end.
    CentreLine,
    Rectangle,
    /// A rectangle from its centre out to a corner.
    CenterRectangle,
    /// A rectangle turned to any angle: one side's two ends, then how wide.
    ThreePointRectangle,
    /// A corner rectangle with its corners rounded at the fillet radius.
    RoundedRectangle,
    Circle,
    /// A circle across a diameter.
    TwoPointCircle,
    /// A circle through three clicked points.
    ThreePointCircle,
    Arc,
    /// An arc from its start to its end, bent through a third point.
    ThreePointArc,
    /// An arc running on smoothly from the end of a line or arc.
    TangentArc,
    /// A regular polygon from its centre out to a corner.
    Polygon,
    /// A regular polygon from its centre out to the middle of a side.
    CircumscribedPolygon,
    /// A slot from one end's centre to the other's, then how wide.
    Slot,
    /// A slot from one rounded end to the other, then how wide.
    OverallSlot,
    /// A slot bent along an arc: the arc's centre, start and end, then how wide.
    ArcSlot,
    /// An ellipse from its centre, the end of its major axis and a point setting its width.
    Ellipse,
    /// A piece of an ellipse: its centre, the end of its major axis, its start and its end.
    EllipseArc,
    /// A spline through the clicked points.
    Spline,
    /// A spline pulled toward the clicked points.
    Bspline,
    Point,
    /// Click a piece of a curve between the curves crossing it to cut it away.
    Trim,
    /// Click near the end of a line or arc to run it on to the next curve.
    Extend,
    /// Click a line or arc to cut it in two there.
    Split,
    /// Click a corner where two lines meet to round it.
    Fillet,
    /// Click a corner where two lines meet to cut it off straight.
    Chamfer,
    /// Click a curve to copy it and the curves joined to it a distance to that side.
    Offset,
    /// Copy the selection across a line.
    Mirror,
    /// Copies of the selection in a row.
    LinearPattern,
    /// Copies of the selection round a centre.
    CircularPattern,
    Move,
    Rotate,
    Scale,
    /// Click what to measure, then where the dimension goes.
    Dimension,
    /// Click the points and curves a geometric constraint goes on.
    Constrain(Rule),
    /// Click regions to make the solid from.
    Regions,
    /// Click the line a revolve turns about.
    Axis,
    /// Click a face of the model to copy its edges into the sketch.
    CopyEdges,
}

/// One tile of the panel: the variants it stands for, the first the one a new editor starts with.
pub(crate) type Family = &'static [Draw];

impl Draw {
    pub(crate) const DRAWING: [Family; 9] = [
        &[Draw::Line, Draw::CentreLine],
        &[Draw::Rectangle, Draw::CenterRectangle, Draw::ThreePointRectangle, Draw::RoundedRectangle],
        &[Draw::Circle, Draw::TwoPointCircle, Draw::ThreePointCircle],
        &[Draw::Arc, Draw::ThreePointArc, Draw::TangentArc],
        &[Draw::Polygon, Draw::CircumscribedPolygon],
        &[Draw::Slot, Draw::OverallSlot, Draw::ArcSlot],
        &[Draw::Ellipse, Draw::EllipseArc],
        &[Draw::Spline, Draw::Bspline],
        &[Draw::Point],
    ];

    pub(crate) const MODIFY: [Family; 6] = [
        &[Draw::Trim, Draw::Extend, Draw::Split],
        &[Draw::Fillet, Draw::Chamfer],
        &[Draw::Offset],
        &[Draw::Mirror],
        &[Draw::LinearPattern, Draw::CircularPattern],
        &[Draw::Move, Draw::Rotate, Draw::Scale],
    ];

    pub(crate) const CONSTRAIN: [Family; 8] = [
        &[Draw::Dimension],
        &[Draw::Constrain(Rule::Coincident), Draw::Constrain(Rule::Midpoint)],
        &[Draw::Constrain(Rule::Horizontal), Draw::Constrain(Rule::Vertical)],
        &[Draw::Constrain(Rule::Parallel), Draw::Constrain(Rule::Perpendicular)],
        &[Draw::Constrain(Rule::Tangent)],
        &[Draw::Constrain(Rule::Equal)],
        &[Draw::Constrain(Rule::Symmetric)],
        &[Draw::Constrain(Rule::Fix)],
    ];

    pub(crate) const PICKING: [Family; 3] = [&[Draw::Regions], &[Draw::Axis], &[Draw::CopyEdges]];

    /// Every family, in panel order.
    pub(crate) fn families() -> impl Iterator<Item = Family> {
        Draw::DRAWING.into_iter().chain(Draw::MODIFY).chain(Draw::CONSTRAIN).chain(Draw::PICKING)
    }

    /// The family the tool belongs to; the pointer is one of its own.
    pub(crate) fn family(self) -> Family {
        Draw::families().find(|family| family.contains(&self)).unwrap_or(&[Draw::Select])
    }

    /// The key that picks the tool's family while sketching, like the single-letter keys of other
    /// CAD programs; these win over the keymap while the sketch is open.
    pub(crate) fn key(self) -> Option<egui::Key> {
        Some(match self.family()[0] {
            Draw::Line => egui::Key::L,
            Draw::Rectangle => egui::Key::R,
            Draw::Circle => egui::Key::C,
            Draw::Arc => egui::Key::A,
            Draw::Polygon => egui::Key::N,
            Draw::Ellipse => egui::Key::E,
            Draw::Spline => egui::Key::S,
            Draw::Point => egui::Key::P,
            Draw::Trim => egui::Key::T,
            Draw::Fillet => egui::Key::F,
            Draw::Offset => egui::Key::O,
            Draw::Mirror => egui::Key::I,
            Draw::Move => egui::Key::M,
            Draw::Dimension => egui::Key::D,
            Draw::Constrain(Rule::Coincident) => egui::Key::X,
            Draw::Constrain(Rule::Horizontal) => egui::Key::H,
            Draw::Constrain(Rule::Equal) => egui::Key::U,
            _ => return None,
        })
    }

    /// Every family with a key, by its first variant.
    pub(crate) fn keyed() -> impl Iterator<Item = (Family, egui::Key)> {
        Draw::families().filter_map(|family| family[0].key().map(|key| (family, key)))
    }

    /// Whether the tool draws new curves from clicks on the plane.
    pub(crate) fn draws(self) -> bool {
        Draw::DRAWING.iter().any(|family| family.contains(&self))
    }

    /// Whether the tool works on the selection: mirroring, patterning and moving it.
    pub(crate) fn uses_selection(self) -> bool {
        matches!(
            self,
            Draw::Mirror | Draw::LinearPattern | Draw::CircularPattern | Draw::Move | Draw::Rotate | Draw::Scale
        )
    }

    /// How many clicks on the plane the selection tools take before they act; the mirror's one
    /// click is on its line instead.
    pub(crate) fn selection_clicks(self) -> usize {
        match self {
            Draw::LinearPattern | Draw::Move => 2,
            Draw::CircularPattern => 1,
            Draw::Rotate | Draw::Scale => 3,
            _ => 0,
        }
    }
}

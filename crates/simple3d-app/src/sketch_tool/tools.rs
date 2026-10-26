//! The editor's tools: what a click in the viewport does, how each is named and explained, and the
//! key that picks it while sketching.

use crate::icon::Glyph;

/// What a click in the viewport does while the sketch is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Draw {
    Select,
    Line,
    Rectangle,
    /// A rectangle from its centre out to a corner.
    CenterRectangle,
    Circle,
    /// A circle through three clicked points.
    ThreePointCircle,
    Arc,
    /// An arc from its start to its end, bent through a third point.
    ThreePointArc,
    /// A regular polygon from its centre out to a corner.
    Polygon,
    /// A spline through the clicked points.
    Spline,
    /// A spline pulled toward the clicked points.
    Bspline,
    Point,
    /// Click a piece of a curve between the curves crossing it to cut it away.
    Trim,
    /// Click a corner where two lines meet to round it.
    Fillet,
    /// Click a curve, or two points, to dimension it.
    Dimension,
    /// Click regions to make the solid from.
    Regions,
    /// Click the line a revolve turns about.
    Axis,
    /// Click a face of the model to copy its edges into the sketch.
    CopyEdges,
}

impl Draw {
    pub(crate) const DRAWING: [Draw; 11] = [
        Draw::Line,
        Draw::Rectangle,
        Draw::CenterRectangle,
        Draw::Circle,
        Draw::ThreePointCircle,
        Draw::Arc,
        Draw::ThreePointArc,
        Draw::Polygon,
        Draw::Spline,
        Draw::Bspline,
        Draw::Point,
    ];

    pub(crate) const MODIFY: [Draw; 3] = [Draw::Trim, Draw::Fillet, Draw::Dimension];

    pub(crate) const PICKING: [Draw; 3] = [Draw::Regions, Draw::Axis, Draw::CopyEdges];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Draw::Select => "Pointer",
            Draw::Line => "Line",
            Draw::Rectangle => "Rectangle",
            Draw::CenterRectangle => "Centre rect",
            Draw::Circle => "Circle",
            Draw::ThreePointCircle => "3-point circle",
            Draw::Arc => "Arc",
            Draw::ThreePointArc => "3-point arc",
            Draw::Polygon => "Polygon",
            Draw::Spline => "Spline",
            Draw::Bspline => "Control spline",
            Draw::Point => "Point",
            Draw::Trim => "Trim",
            Draw::Fillet => "Fillet",
            Draw::Dimension => "Dimension",
            Draw::Regions => "Pick regions",
            Draw::Axis => "Revolve axis",
            Draw::CopyEdges => "Copy face edges",
        }
    }

    /// The tool's tile in the panel.
    pub(crate) fn glyph(self) -> Glyph {
        match self {
            Draw::Select => Glyph::SketchSelect,
            Draw::Line => Glyph::SketchLine,
            Draw::Rectangle => Glyph::SketchRectangle,
            Draw::CenterRectangle => Glyph::SketchCentreRectangle,
            Draw::Circle => Glyph::SketchCircle,
            Draw::ThreePointCircle => Glyph::SketchThreePointCircle,
            Draw::Arc => Glyph::SketchArc,
            Draw::ThreePointArc => Glyph::SketchThreePointArc,
            Draw::Polygon => Glyph::SketchPolygon,
            Draw::Spline => Glyph::SketchSpline,
            Draw::Bspline => Glyph::SketchControlSpline,
            Draw::Point => Glyph::SketchPoint,
            Draw::Trim => Glyph::SketchTrim,
            Draw::Fillet => Glyph::SketchFillet,
            Draw::Dimension => Glyph::SketchDimension,
            Draw::Regions => Glyph::SketchRegions,
            Draw::Axis => Glyph::SketchAxis,
            Draw::CopyEdges => Glyph::SketchCopyEdges,
        }
    }

    /// The tool's name with the key that picks it, for its tooltip and the panel's hint line.
    pub(crate) fn named(self) -> String {
        match self.key() {
            Some(key) => format!("{} ({})", self.label(), key.name()),
            None if self == Draw::Select => format!("{} (Esc)", self.label()),
            None => self.label().to_string(),
        }
    }

    /// What to do next with this tool, for the window's hint line.
    pub(crate) fn hint(self) -> &'static str {
        match self {
            Draw::Select => {
                "Click points, curves or constraints to select them and change them in place, Shift adds; drag to \
                 move."
            }
            Draw::Line => "Click to put lines down one after another; right-click or Esc ends the chain.",
            Draw::Rectangle => "Click two opposite corners.",
            Draw::CenterRectangle => "Click the centre, then a corner.",
            Draw::Circle => "Click the centre, then a point on the circle.",
            Draw::ThreePointCircle => "Click three points the circle runs through.",
            Draw::Arc => "Click the centre, the start, then the end; the arc runs counter-clockwise.",
            Draw::ThreePointArc => "Click the start, the end, then a point the arc bends through.",
            Draw::Polygon => "Click the centre, then a corner; the number of sides is set below.",
            Draw::Spline => "Click the points it passes through; right-click ends it, a click on the first closes it.",
            Draw::Bspline => "Click the points that pull on it; right-click ends it, a click on the first closes it.",
            Draw::Point => "Click to put a point down.",
            Draw::Trim => "Click the piece of a line, arc or circle to cut away, up to the curves crossing it.",
            Draw::Fillet => "Click a corner where two lines meet to round it with the radius below.",
            Draw::Dimension => {
                "Click a line or a circle to dimension it, or two points, or a point and then a line, for the \
                 distance between them."
            }
            Draw::Regions => "Click the closed areas to make the solid from; click again to drop one.",
            Draw::Axis => "Click the line to revolve about.",
            Draw::CopyEdges => "Click a face of the model to copy its edges onto the sketch.",
        }
    }

    /// The key that picks the tool while sketching, like the single-letter keys of other CAD
    /// programs; these win over the keymap while the sketch is open.
    pub(crate) fn key(self) -> Option<egui::Key> {
        Some(match self {
            Draw::Line => egui::Key::L,
            Draw::Rectangle => egui::Key::R,
            Draw::Circle => egui::Key::C,
            Draw::Arc => egui::Key::A,
            Draw::Polygon => egui::Key::N,
            Draw::Spline => egui::Key::S,
            Draw::Point => egui::Key::P,
            Draw::Trim => egui::Key::T,
            Draw::Fillet => egui::Key::F,
            Draw::Dimension => egui::Key::D,
            _ => return None,
        })
    }

    /// Every tool with a key.
    pub(crate) fn keyed() -> impl Iterator<Item = (Draw, egui::Key)> {
        Draw::DRAWING
            .into_iter()
            .chain(Draw::MODIFY)
            .chain(Draw::PICKING)
            .filter_map(|draw| draw.key().map(|key| (draw, key)))
    }

    /// Whether the tool draws new curves from clicks on the plane.
    pub(crate) fn draws(self) -> bool {
        Draw::DRAWING.contains(&self)
    }
}

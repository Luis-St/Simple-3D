//! How each tool is named, explained and drawn on its tile.

use super::Draw;
use crate::icon::Glyph;

impl Draw {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Draw::Select => "Pointer",
            Draw::Line => "Line",
            Draw::CentreLine => "Centre line",
            Draw::Rectangle => "Rectangle",
            Draw::CenterRectangle => "Centre rect",
            Draw::ThreePointRectangle => "3-point rect",
            Draw::RoundedRectangle => "Rounded rect",
            Draw::Circle => "Circle",
            Draw::TwoPointCircle => "2-point circle",
            Draw::ThreePointCircle => "3-point circle",
            Draw::Arc => "Arc",
            Draw::ThreePointArc => "3-point arc",
            Draw::TangentArc => "Tangent arc",
            Draw::Polygon => "Polygon",
            Draw::CircumscribedPolygon => "Outer polygon",
            Draw::Slot => "Slot",
            Draw::OverallSlot => "Overall slot",
            Draw::ArcSlot => "Arc slot",
            Draw::Ellipse => "Ellipse",
            Draw::EllipseArc => "Elliptical arc",
            Draw::Spline => "Spline",
            Draw::Bspline => "Control spline",
            Draw::Point => "Point",
            Draw::Trim => "Trim",
            Draw::Extend => "Extend",
            Draw::Split => "Split",
            Draw::Fillet => "Fillet",
            Draw::Chamfer => "Chamfer",
            Draw::Offset => "Offset",
            Draw::Mirror => "Mirror",
            Draw::LinearPattern => "Linear pattern",
            Draw::CircularPattern => "Circular pattern",
            Draw::Move => "Move",
            Draw::Rotate => "Rotate",
            Draw::Scale => "Scale",
            Draw::Dimension => "Dimension",
            Draw::Constrain(rule) => rule.label(),
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
            Draw::CentreLine => Glyph::SketchCentreLine,
            Draw::Rectangle => Glyph::SketchRectangle,
            Draw::CenterRectangle => Glyph::SketchCentreRectangle,
            Draw::ThreePointRectangle => Glyph::SketchThreePointRectangle,
            Draw::RoundedRectangle => Glyph::SketchRoundedRectangle,
            Draw::Circle => Glyph::SketchCircle,
            Draw::TwoPointCircle => Glyph::SketchTwoPointCircle,
            Draw::ThreePointCircle => Glyph::SketchThreePointCircle,
            Draw::Arc => Glyph::SketchArc,
            Draw::ThreePointArc => Glyph::SketchThreePointArc,
            Draw::TangentArc => Glyph::SketchTangentArc,
            Draw::Polygon => Glyph::SketchPolygon,
            Draw::CircumscribedPolygon => Glyph::SketchOuterPolygon,
            Draw::Slot => Glyph::SketchSlot,
            Draw::OverallSlot => Glyph::SketchOverallSlot,
            Draw::ArcSlot => Glyph::SketchArcSlot,
            Draw::Ellipse => Glyph::SketchEllipse,
            Draw::EllipseArc => Glyph::SketchEllipseArc,
            Draw::Spline => Glyph::SketchSpline,
            Draw::Bspline => Glyph::SketchControlSpline,
            Draw::Point => Glyph::SketchPoint,
            Draw::Trim => Glyph::SketchTrim,
            Draw::Extend => Glyph::SketchExtend,
            Draw::Split => Glyph::SketchSplit,
            Draw::Fillet => Glyph::SketchFillet,
            Draw::Chamfer => Glyph::SketchChamfer,
            Draw::Offset => Glyph::SketchOffset,
            Draw::Mirror => Glyph::SketchMirror,
            Draw::LinearPattern => Glyph::SketchLinearPattern,
            Draw::CircularPattern => Glyph::SketchCircularPattern,
            Draw::Move => Glyph::SketchMove,
            Draw::Rotate => Glyph::SketchRotate,
            Draw::Scale => Glyph::SketchScale,
            Draw::Dimension => Glyph::SketchDimension,
            Draw::Constrain(rule) => rule.glyph(),
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
                "Click points, curves or constraints to select them and change them in place, Shift adds without \
                 opening the popup; drag to move, or from empty space to select by area."
            }
            Draw::Line => "Click to put lines down one after another; right-click or Esc ends the chain.",
            Draw::CentreLine => "Click the middle, then one end; the other end mirrors it.",
            Draw::Rectangle => "Click two opposite corners.",
            Draw::CenterRectangle => "Click the centre, then a corner.",
            Draw::ThreePointRectangle => "Click both ends of one side, then how far the opposite side lies.",
            Draw::RoundedRectangle => "Click two opposite corners; the corners are rounded with the radius below.",
            Draw::Circle => "Click the centre, then a point on the circle.",
            Draw::TwoPointCircle => "Click both ends of a diameter.",
            Draw::ThreePointCircle => "Click three points the circle runs through.",
            Draw::Arc => "Click the centre, the start, then the end; the arc runs counter-clockwise.",
            Draw::ThreePointArc => "Click the start, the end, then a point the arc bends through.",
            Draw::TangentArc => {
                "Click the end of a line or arc, then where the arc ends; it runs on smoothly, and right-click ends \
                 the chain."
            }
            Draw::Polygon => "Click the centre, then a corner; the number of sides is set below.",
            Draw::CircumscribedPolygon => {
                "Click the centre, then the middle of a side; the number of sides is set below."
            }
            Draw::Slot => "Click the centres of both rounded ends, then how wide.",
            Draw::OverallSlot => "Click both outer ends, then how wide.",
            Draw::ArcSlot => "Click the arc's centre, its start and its end, then how wide.",
            Draw::Ellipse => "Click the centre, the end of the long axis, then how wide.",
            Draw::EllipseArc => "Click the centre, the end of the long axis, the start, then the end.",
            Draw::Spline => "Click the points it passes through; right-click ends it, a click on the first closes it.",
            Draw::Bspline => "Click the points that pull on it; right-click ends it, a click on the first closes it.",
            Draw::Point => "Click to put a point down.",
            Draw::Trim => "Click the piece of a line, arc or circle to cut away, up to the curves crossing it.",
            Draw::Extend => "Click near the end of a line or arc to run it on to the next curve it meets.",
            Draw::Split => "Click a line or arc where it is to be cut in two.",
            Draw::Fillet => "Click a corner where two lines meet to round it with the radius below.",
            Draw::Chamfer => "Click a corner where two lines meet to cut it off the distance below along both.",
            Draw::Offset => "Click a curve on the side to copy it to; the curves joined to it come along.",
            Draw::Mirror => "Select what to mirror (Shift-click adds here), then click the line to mirror across.",
            Draw::LinearPattern => "Select what to copy, then click a start and where the first copy goes.",
            Draw::CircularPattern => "Select what to copy, then click the centre to copy it round.",
            Draw::Move => "Select what to move, then click a start and where it goes.",
            Draw::Rotate => "Select what to turn, then click the centre, a start and where it turns to.",
            Draw::Scale => "Select what to scale, then click the centre, a start and where it grows to.",
            Draw::Dimension => {
                "Click a line, a circle or arc, two points, a point and a line, or two lines for their angle, then \
                 click where the dimension goes; beside a slanted line it measures across or up."
            }
            Draw::Constrain(rule) => rule.hint(),
            Draw::Regions => "Click the closed areas to make the solid from; click again to drop one.",
            Draw::Axis => "Click the line to revolve about.",
            Draw::CopyEdges => "Click a face of the model to copy its edges onto the sketch.",
        }
    }
}

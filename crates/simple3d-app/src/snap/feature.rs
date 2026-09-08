//! What can be snapped to: a vertex, an edge or a face.

use simple3d_geom::Vec3;

/// Which of the three notable points a feature is. Kept so the interface can
/// name what a snap or a measurement caught, and so a preference could one day
/// turn a kind off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureKind {
    Vertex,
    EdgeMidpoint,
    FaceCentre,
    /// Anywhere along an edge, rather than one of its notable points. Not a
    /// feature the list carries -- it is what catching an edge *between* its
    /// ends reports (issue 78), so a measurement can start part-way along one.
    Edge,
    /// Anywhere along a world axis. The axes are lines, and a point on one is
    /// as real a place to measure from as a point on an edge, so they are caught
    /// the same way rather than only at the handful of places where they meet
    /// something.
    Axis,
    /// Anywhere along the line a principal plane leaves on a body's surface.
    ///
    /// The renderer draws, on the solid itself, where each plane through the
    /// origin cuts it -- the mark that says how much of the shape is below the
    /// build plate, or which side of zero a face is on. It is a line on the
    /// object, in the colour of the axis its plane is named by, and every other
    /// line in the picture can be caught, so this one is too.
    PlaneMark,
    /// Where a world axis passes through a body's surface. The axes run through
    /// the model whether or not any geometry corner is there, and a corner of the
    /// model on an axis is exactly the place a measurement usually wants, so the
    /// crossings are offered as corners and the run between two of them as an
    /// edge (issue 78).
    AxisCrossing,
}

impl FeatureKind {
    pub fn label(self) -> &'static str {
        match self {
            FeatureKind::Vertex => "vertex",
            FeatureKind::EdgeMidpoint => "edge midpoint",
            FeatureKind::FaceCentre => "face centre",
            FeatureKind::Edge => "edge",
            FeatureKind::Axis => "axis",
            FeatureKind::PlaneMark => "plane mark",
            FeatureKind::AxisCrossing => "axis crossing",
        }
    }
}

/// One catchable point on a body, in world space.
///
/// An edge feature also carries the edge it is the middle of, so a pointer that
/// is near the edge but nowhere near its midpoint can still catch the edge --
/// at the place along it that is actually being pointed at (issue 78).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feature {
    pub point: Vec3,
    pub kind: FeatureKind,
    pub span: Option<(Vec3, Vec3)>,
}

impl Feature {
    /// A feature that is a point and nothing more.
    pub fn point(point: Vec3, kind: FeatureKind) -> Feature {
        Feature { point, kind, span: None }
    }

    /// An edge, reported at its midpoint and carrying its two ends.
    pub fn edge(a: Vec3, b: Vec3) -> Feature {
        Feature { point: (a + b) * 0.5, kind: FeatureKind::EdgeMidpoint, span: Some((a, b)) }
    }
}

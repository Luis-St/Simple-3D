//! What can be snapped to: a vertex, an edge or a face.

use simple3d_geom::Vec3;

/// Which kind of point a feature is, so the interface can name what was caught.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureKind {
    Vertex,
    EdgeMidpoint,
    FaceCentre,
    /// Anywhere along an edge rather than a notable point: what catching an edge between its ends
    /// reports (issue 78). Not carried in the feature list.
    Edge,
    /// Anywhere along a world axis, caught like any other line.
    Axis,
    /// Anywhere along a principal plane's mark on a body's surface, drawn and so catchable like any line.
    PlaneMark,
    /// Where a world axis passes through a body's surface: offered as corners, with the run between
    /// two as an edge (issue 78).
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

/// One catchable point on a body in world space. An edge feature also carries its ends, so it can
/// be caught anywhere along it (issue 78).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feature {
    pub point: Vec3,
    pub kind: FeatureKind,
    pub span: Option<(Vec3, Vec3)>,
}

impl Feature {
    /// A feature that is only a point.
    pub fn point(point: Vec3, kind: FeatureKind) -> Feature {
        Feature { point, kind, span: None }
    }

    /// An edge, reported at its midpoint and carrying its two ends.
    pub fn edge(a: Vec3, b: Vec3) -> Feature {
        Feature { point: (a + b) * 0.5, kind: FeatureKind::EdgeMidpoint, span: Some((a, b)) }
    }
}

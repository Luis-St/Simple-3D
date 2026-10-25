//! A sketch curve: what is drawn between and around points.

use crate::point::{is_false, PointId};
use serde::{Deserialize, Serialize};

pub type CurveId = u32;

/// The curve's kind and the points it is drawn from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shape {
    Line {
        a: PointId,
        b: PointId,
    },
    /// Counter-clockwise from `start` to `end` about `center`. Its radius is `start`'s distance from
    /// the centre; the solver keeps `end` at the same distance.
    Arc {
        center: PointId,
        start: PointId,
        end: PointId,
    },
    /// Its radius is a solver unknown of its own, since no point lies on it.
    Circle {
        center: PointId,
        radius: f64,
    },
    /// A smooth curve through every one of `points` (a centripetal Catmull-Rom spline), closed back
    /// onto the first when `closed`.
    Spline {
        points: Vec<PointId>,
        #[serde(default, skip_serializing_if = "is_false")]
        closed: bool,
    },
    /// A cubic B-spline its `points` pull on without lying on: it starts on the first and ends on the
    /// last when open, and runs round them all when `closed`.
    Bspline {
        points: Vec<PointId>,
        #[serde(default, skip_serializing_if = "is_false")]
        closed: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    pub id: CurveId,
    pub shape: Shape,
    /// Helper geometry: constrained against and used as a revolve axis, but never the edge of a
    /// region. Omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub construction: bool,
    /// Copied in from an edge of the model rather than drawn; its points are fixed. Omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reference: bool,
}

impl Shape {
    /// Every point the curve is drawn from, centre included.
    pub fn points(&self) -> Vec<PointId> {
        match self {
            Shape::Line { a, b } => vec![*a, *b],
            Shape::Arc { center, start, end } => vec![*center, *start, *end],
            Shape::Circle { center, .. } => vec![*center],
            Shape::Spline { points, .. } | Shape::Bspline { points, .. } => points.clone(),
        }
    }

    /// The points the curve begins and ends on, if it has ends: what joins it to its neighbours.
    pub fn ends(&self) -> Option<(PointId, PointId)> {
        match self {
            Shape::Line { a, b } => Some((*a, *b)),
            Shape::Arc { start, end, .. } => Some((*start, *end)),
            Shape::Circle { .. } => None,
            Shape::Spline { points, closed } => (!closed).then(|| (points[0], points[points.len() - 1])),
            Shape::Bspline { points, closed } => (!closed).then(|| (points[0], points[points.len() - 1])),
        }
    }

    /// Whether the curve is round (an arc or a circle), which radius and tangent constraints need.
    pub fn is_round(&self) -> bool {
        matches!(self, Shape::Arc { .. } | Shape::Circle { .. })
    }

    pub fn is_line(&self) -> bool {
        matches!(self, Shape::Line { .. })
    }

    pub fn is_spline(&self) -> bool {
        matches!(self, Shape::Spline { .. } | Shape::Bspline { .. })
    }

    /// The word for it, for the status line and the panel.
    pub fn label(&self) -> &'static str {
        match self {
            Shape::Line { .. } => "line",
            Shape::Arc { .. } => "arc",
            Shape::Circle { .. } => "circle",
            Shape::Spline { .. } => "spline",
            Shape::Bspline { .. } => "control-point spline",
        }
    }
}

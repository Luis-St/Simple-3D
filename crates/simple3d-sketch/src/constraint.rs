//! Geometric constraints and dimensions.

use crate::curve::CurveId;
use crate::point::PointId;
use serde::{Deserialize, Serialize};

pub type ConstraintId = u32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: ConstraintId,
    #[serde(flatten)]
    pub kind: ConstraintKind,
}

/// What a constraint holds. The ones with a `value` are dimensions: lengths in millimetres, angles
/// in degrees, typed by the user and shown as labels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstraintKind {
    Coincident {
        a: PointId,
        b: PointId,
    },
    Horizontal {
        line: CurveId,
    },
    Vertical {
        line: CurveId,
    },
    Parallel {
        a: CurveId,
        b: CurveId,
    },
    Perpendicular {
        a: CurveId,
        b: CurveId,
    },
    /// A line and a round curve touching, two round curves touching (from inside when `inside`), or
    /// two curves sharing an end point leaving it in one direction, splines included.
    Tangent {
        a: CurveId,
        b: CurveId,
        #[serde(default)]
        inside: bool,
    },
    /// Two lines of one length, or two round curves of one radius.
    Equal {
        a: CurveId,
        b: CurveId,
    },
    /// `a` and `b` mirror images across `line`.
    Symmetric {
        a: PointId,
        b: PointId,
        line: CurveId,
    },
    Midpoint {
        point: PointId,
        line: CurveId,
    },
    /// A point on a line (extended past its ends) or on a round curve.
    OnCurve {
        point: PointId,
        curve: CurveId,
    },
    /// A point held where it is.
    Fix {
        point: PointId,
        at: [f64; 2],
    },
    Distance {
        a: PointId,
        b: PointId,
        value: f64,
    },
    /// The distance along X only; kept on whichever side `b` was when it was set.
    HorizontalDistance {
        a: PointId,
        b: PointId,
        value: f64,
    },
    VerticalDistance {
        a: PointId,
        b: PointId,
        value: f64,
    },
    /// A point's distance from a line, extended past its ends.
    PointLineDistance {
        point: PointId,
        line: CurveId,
        value: f64,
    },
    Radius {
        curve: CurveId,
        value: f64,
    },
    /// The angle from line `a` to line `b`, in degrees.
    Angle {
        a: CurveId,
        b: CurveId,
        value: f64,
    },
}

impl ConstraintKind {
    /// The points it names directly.
    pub fn points(&self) -> Vec<PointId> {
        use ConstraintKind::*;
        match self {
            Coincident { a, b }
            | Distance { a, b, .. }
            | HorizontalDistance { a, b, .. }
            | VerticalDistance { a, b, .. } => vec![*a, *b],
            Symmetric { a, b, .. } => vec![*a, *b],
            Midpoint { point, .. } | OnCurve { point, .. } | PointLineDistance { point, .. } => vec![*point],
            Fix { point, .. } => vec![*point],
            _ => Vec::new(),
        }
    }

    /// The curves it names directly.
    pub fn curves(&self) -> Vec<CurveId> {
        use ConstraintKind::*;
        match self {
            Horizontal { line } | Vertical { line } => vec![*line],
            Parallel { a, b } | Perpendicular { a, b } | Tangent { a, b, .. } | Equal { a, b } | Angle { a, b, .. } => {
                vec![*a, *b]
            }
            Symmetric { line, .. } | Midpoint { line, .. } | PointLineDistance { line, .. } => vec![*line],
            OnCurve { curve, .. } | Radius { curve, .. } => vec![*curve],
            _ => Vec::new(),
        }
    }

    /// A dimension's value, or `None` for a plain geometric constraint.
    pub fn value(&self) -> Option<f64> {
        use ConstraintKind::*;
        match self {
            Distance { value, .. }
            | HorizontalDistance { value, .. }
            | VerticalDistance { value, .. }
            | PointLineDistance { value, .. }
            | Radius { value, .. }
            | Angle { value, .. } => Some(*value),
            _ => None,
        }
    }

    pub fn value_mut(&mut self) -> Option<&mut f64> {
        use ConstraintKind::*;
        match self {
            Distance { value, .. }
            | HorizontalDistance { value, .. }
            | VerticalDistance { value, .. }
            | PointLineDistance { value, .. }
            | Radius { value, .. }
            | Angle { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Whether the value is an angle rather than a length.
    pub fn is_angle(&self) -> bool {
        matches!(self, ConstraintKind::Angle { .. })
    }

    /// The word for it, for the status line and the constraint list.
    pub fn label(&self) -> &'static str {
        use ConstraintKind::*;
        match self {
            Coincident { .. } => "Coincident",
            Horizontal { .. } => "Horizontal",
            Vertical { .. } => "Vertical",
            Parallel { .. } => "Parallel",
            Perpendicular { .. } => "Perpendicular",
            Tangent { .. } => "Tangent",
            Equal { .. } => "Equal",
            Symmetric { .. } => "Symmetric",
            Midpoint { .. } => "Midpoint",
            OnCurve { .. } => "On curve",
            Fix { .. } => "Fixed",
            Distance { .. } => "Distance",
            HorizontalDistance { .. } => "Horizontal distance",
            VerticalDistance { .. } => "Vertical distance",
            PointLineDistance { .. } => "Distance from line",
            Radius { .. } => "Radius",
            Angle { .. } => "Angle",
        }
    }
}

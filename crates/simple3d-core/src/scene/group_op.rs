//! What a group does with its children, and where a shape's origin sits.

use serde::{Deserialize, Serialize};
use simple3d_geom::BooleanOp;

/// Where a node's origin sits (spec section 3.1); changing it moves the origin, never the shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    #[default]
    Centre,
    /// Minimum Z at the origin, for standing things on a build plate.
    Base,
}

impl Anchor {
    pub const ALL: [Anchor; 2] = [Anchor::Centre, Anchor::Base];

    pub fn label(self) -> &'static str {
        match self {
            Anchor::Centre => "Centre",
            Anchor::Base => "Base",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupOp {
    #[default]
    Union,
    Difference,
    Intersection,
    Hull,
    /// No boolean: children side by side, each its own body. Made for multi-object files whose parts
    /// share faces, where a union is the kernel's slowest case and still fails.
    Assembly,
}

impl GroupOp {
    pub const ALL: [GroupOp; 5] =
        [GroupOp::Union, GroupOp::Difference, GroupOp::Intersection, GroupOp::Hull, GroupOp::Assembly];

    pub fn label(self) -> &'static str {
        match self {
            GroupOp::Union => "Union",
            GroupOp::Difference => "Difference",
            GroupOp::Intersection => "Intersection",
            GroupOp::Hull => "Hull",
            GroupOp::Assembly => "Assembly",
        }
    }

    /// Difference is the only operation where child order matters.
    pub fn order_matters(self) -> bool {
        self == GroupOp::Difference
    }

    /// Whether the result still contains its operands as separable pieces: union and assembly yes;
    /// difference, intersection and hull make one new surface, so their children cannot be export bodies.
    pub fn separable(self) -> bool {
        matches!(self, GroupOp::Union | GroupOp::Assembly)
    }

    /// The kernel's operation, or `None` for the one group that runs none.
    pub fn to_geom(self) -> Option<BooleanOp> {
        match self {
            GroupOp::Union => Some(BooleanOp::Union),
            GroupOp::Difference => Some(BooleanOp::Difference),
            GroupOp::Intersection => Some(BooleanOp::Intersection),
            GroupOp::Hull => Some(BooleanOp::Hull),
            GroupOp::Assembly => None,
        }
    }
}

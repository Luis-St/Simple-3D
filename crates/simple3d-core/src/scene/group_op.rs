//! What a group does with its children, and where a shape's origin sits.

use serde::{Deserialize, Serialize};
use simple3d_geom::BooleanOp;

/// Where a node's origin sits (spec section 3.1). Changing it moves the origin,
/// never the shape.
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
}

impl GroupOp {
    pub const ALL: [GroupOp; 4] = [GroupOp::Union, GroupOp::Difference, GroupOp::Intersection, GroupOp::Hull];

    pub fn label(self) -> &'static str {
        match self {
            GroupOp::Union => "Union",
            GroupOp::Difference => "Difference",
            GroupOp::Intersection => "Intersection",
            GroupOp::Hull => "Hull",
        }
    }

    /// Difference is the only operation where child order carries meaning.
    pub fn order_matters(self) -> bool {
        self == GroupOp::Difference
    }

    /// Whether the result still contains its operands as pieces that can be
    /// taken out of it. A union is its operands standing side by side; a
    /// difference, an intersection and a hull are one new surface, and a child
    /// of one of those is not a solid that exists in the result at all -- so it
    /// can never be a body of its own in an export.
    pub fn separable(self) -> bool {
        self == GroupOp::Union
    }

    pub fn to_geom(self) -> BooleanOp {
        match self {
            GroupOp::Union => BooleanOp::Union,
            GroupOp::Difference => BooleanOp::Difference,
            GroupOp::Intersection => BooleanOp::Intersection,
            GroupOp::Hull => BooleanOp::Hull,
        }
    }
}

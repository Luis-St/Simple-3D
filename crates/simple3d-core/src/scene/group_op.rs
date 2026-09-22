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
    /// No boolean at all: the children side by side, each still a body of its
    /// own -- what a split does with its pieces, for a group of shapes that
    /// were never one.
    ///
    /// Made for a file of several objects. A 3MF holding a model's colours as
    /// separate objects has them meeting along whole faces, and a union of two
    /// bodies that share a face is the kernel's worst case: the parts of one
    /// imported model took minutes to union, the union came out non-manifold
    /// all the same, and the group fell back to showing them side by side with
    /// an error on it -- which is where it should have started.
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

    /// Difference is the only operation where child order carries meaning.
    pub fn order_matters(self) -> bool {
        self == GroupOp::Difference
    }

    /// Whether the result still contains its operands as pieces that can be
    /// taken out of it. A union is its operands standing side by side, and an
    /// assembly is nothing else; a difference, an intersection and a hull are
    /// one new surface, and a child of one of those is not a solid that exists
    /// in the result at all -- so it can never be a body of its own in an
    /// export.
    pub fn separable(self) -> bool {
        matches!(self, GroupOp::Union | GroupOp::Assembly)
    }

    /// The kernel's operation, and `None` for the one group that runs none.
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

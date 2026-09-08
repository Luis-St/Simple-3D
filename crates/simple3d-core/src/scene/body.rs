//! What a node *is*: a primitive, a group, a pattern, a mesh or a split,
//! and whether it is an object of its own on export.

use super::*;
use crate::mesh_data::MeshData;
use crate::primitive::Params;
use serde::{Deserialize, Serialize};
use simple3d_geom::tiling::SplitPlan;
use std::sync::Arc;

/// What a node is in an export that lets the user choose its bodies (issue 58).
///
/// The absence of one -- `Node::export_body` being `None` -- means "a body of
/// its own", which is what every node is until it is told otherwise. An
/// untouched project therefore exports exactly as "top level bodies" does, and
/// a mark is only ever needed where the answer differs from that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportBody {
    /// Merged, as one solid, with every other node carrying the same number.
    Shared(u32),
    /// Not a body itself: its children are each considered in its place, which
    /// is how an export reaches inside a group. Only a separable group can
    /// carry this -- see [`GroupOp::separable`].
    Split,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Group {
        op: GroupOp,
    },
    Primitive {
        type_id: String,
        params: Params,
    },
    /// A node that repeats its children under a rule (issue 67). It holds
    /// children like a group, and its parameters -- the kind of pattern and its
    /// numbers -- ride in the same `Params` map a primitive uses.
    Pattern {
        params: Params,
    },
    /// Geometry the node owns outright, with no recipe behind it -- what a
    /// shape becomes when it is converted to a mesh (issue 80).
    ///
    /// Behind an `Arc` because undo snapshots the whole scene, and a converted
    /// body of any size is megabytes: sharing the triangles between every
    /// snapshot that did not touch them is what keeps the history affordable.
    Mesh {
        mesh: Arc<MeshData>,
    },
    /// What breaking a shape apart leaves behind (issue 82): a node holding the
    /// separate pieces the shape was actually in, as children, and the object it
    /// was made from, so it can be put back together.
    ///
    /// It combines its children exactly as a union group does -- the pieces
    /// stand side by side, which is what they did inside the shape -- but it is
    /// a body of its own rather than a group, because a group is something the
    /// user assembles and this is something the application made *out of* one
    /// object. That is also why nothing offers to create one: it exists only
    /// where "split into smaller pieces" put it.
    ///
    /// `original` is the whole subtree the pieces came from, in the portable
    /// form the project file and the clipboard already use, behind an `Arc` for
    /// the same reason a stored mesh is: undo snapshots the scene, and the
    /// recipe must not be copied into every snapshot that never touched it.
    ///
    /// `plan` is how the pieces were made: the cell shapes and their numbers,
    /// one entry per cut, for a shape cut into a pattern of them -- and `None`
    /// for one merely separated into the pieces it was already in. It is a
    /// label and an offer, not a recipe -- the pieces are geometry and nothing
    /// re-derives them from it -- but it is what lets the panel say what was
    /// done and lets the tool open again on the numbers that did it.
    Split {
        original: Arc<NodeData>,
        plan: Option<SplitPlan>,
    },
}

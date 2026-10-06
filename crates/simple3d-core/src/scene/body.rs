//! What a node is (primitive, group, pattern, mesh or split), and whether it is its own export body.

use super::*;
use crate::mesh_data::MeshData;
use crate::primitive::Params;
use serde::{Deserialize, Serialize};
use simple3d_geom::push_pull::Outline;
use simple3d_geom::tiling::SplitPlan;
use std::sync::Arc;

/// A node's role in an export with chosen bodies (issue 58). `None` means a body of its own, so an
/// untouched project exports as "top level bodies".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportBody {
    /// Merged into one solid with every node carrying the same number.
    Shared(u32),
    /// Not a body itself: its children are considered in its place. Only separable groups
    /// ([`GroupOp::separable`]).
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
    /// Repeats its children under a rule (issue 67), with its parameters in a `Params` map.
    Pattern {
        params: Params,
    },
    /// Geometry owned outright with no recipe: a converted shape (issue 80). Behind an `Arc` so undo
    /// snapshots share it.
    Mesh {
        mesh: Arc<MeshData>,
    },
    /// What breaking a shape apart leaves (issue 82): the pieces as children plus the original, so it
    /// can be joined back. Combines like a union, but only the split tool creates it.
    ///
    /// `original` is the source subtree in portable form, behind an `Arc` so undo snapshots share it.
    /// `plan` records the cuts (or `None` for a plain separation): a label and a way to reopen the
    /// tool, not a recipe.
    Split {
        original: Arc<NodeData>,
        plan: Option<SplitPlan>,
    },
    /// A solid pushed out of, or into, a face of the model (issue 73): the face's outline as captured,
    /// in the node's own XY plane, swept along its +Z by the `distance` parameter. Behind an `Arc` so
    /// undo snapshots share it.
    Extrusion {
        outline: Arc<Outline>,
        params: Params,
    },
    /// A component placed here (issue 113), standing for another component's whole tree. `op`, if set,
    /// overrides the component's own operation; placement, visibility and colour are the node's own.
    Component {
        component: ComponentId,
        op: Option<GroupOp>,
    },
}

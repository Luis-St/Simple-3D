//! The stored form of a node: what a project file holds and what it leaves
//! out when it is at its default.

use super::*;
use crate::mesh_data::MeshBlob;
use crate::primitive::Params;
use serde::{Deserialize, Serialize};
use simple3d_geom::tiling::SplitPlan;
use simple3d_geom::Vec3;

/// The readable, diffable form of a node used by both the project file and the
/// clipboard, so a selection can be pasted into a text editor and back again
/// (spec sections 8.1, 10).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    pub name: String,
    #[serde(rename = "type")]
    pub type_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<GroupOp>,
    #[serde(default = "Vec3_zero")]
    pub position: Vec3,
    #[serde(default = "Vec3_zero")]
    pub rotation: Vec3,
    /// Left out of the file entirely when it is `1, 1, 1`, which is almost
    /// always, so a project written by this version still diffs cleanly against
    /// one written before scale existed.
    #[serde(default = "Vec3_one", skip_serializing_if = "is_unit_scale")]
    pub scale: Vec3,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Absent from the file for every node that is not a ghost, which is nearly
    /// all of them.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ghost: bool,
    /// `#rrggbb`, and absent from the file for the usual unpainted node, so a
    /// project written by this version still diffs cleanly against one written
    /// before colours existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<u32>,
    /// Absent from the file for every node that is a body of its own, which is
    /// every node until an export is told to group them differently -- so a
    /// project written by this version still diffs cleanly against one written
    /// before export bodies existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_body: Option<ExportBody>,
    /// Whether a piece has been lifted out of the collection holding it, so
    /// that it has a row of its own in the tree (issue 82). Absent from the
    /// file for every node that is not one -- which is every node in a project
    /// that has never split anything -- so a project written by this version
    /// still diffs cleanly against one written before collections existed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub extracted: bool,
    /// The geometry of a `mesh` node, and nothing else's. Present only on the
    /// one body type that owns its triangles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<MeshBlob>,
    /// The object a `split` node was broken apart from, and nothing else's:
    /// the whole subtree, so joining the pieces back together rebuilds the
    /// shape with its operands and its parameters intact (issue 82).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<Box<NodeData>>,
    /// How a `split` node's pieces were cut, and nothing else's: one entry per
    /// cut, each naming a cell shape and its numbers (issue 82). Absent for a
    /// split that merely separated a shape into the pieces it was already in,
    /// and for every other node -- so a project written by this version still
    /// diffs cleanly against one written before a split could be a pattern of
    /// cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiling: Option<SplitPlan>,
    #[serde(default, skip_serializing_if = "Params::is_empty")]
    pub params: Params,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeData>,
}

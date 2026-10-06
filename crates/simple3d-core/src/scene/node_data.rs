//! The stored form of a node: what a project file holds, omitting defaults.

use super::*;
use crate::mesh_data::MeshBlob;
use crate::primitive::Params;
use serde::{Deserialize, Serialize};
use simple3d_geom::push_pull::Outline;
use simple3d_geom::tiling::SplitPlan;
use simple3d_geom::Vec3;

/// The readable node form shared by project files and the clipboard (spec sections 8.1, 10).
/// Newer fields are omitted at their defaults so files diff cleanly against older ones.
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
    /// Omitted when `1, 1, 1`.
    #[serde(default = "Vec3_one", skip_serializing_if = "is_unit_scale")]
    pub scale: Vec3,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Omitted unless a ghost.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ghost: bool,
    /// `#rrggbb`, omitted when unpainted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<u32>,
    /// Omitted for a body of its own, the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_body: Option<ExportBody>,
    /// Whether a piece has been lifted out of its collection (issue 82); omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub extracted: bool,
    /// A `mesh` node's geometry; only on that body type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<MeshBlob>,
    /// A `split` node's source subtree, so joining rebuilds it intact (issue 82).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<Box<NodeData>>,
    /// A `split` node's cuts, one per pass (issue 82); absent for a plain separation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiling: Option<SplitPlan>,
    /// An `extrusion` node's captured outline (issue 73); its distance is in `params`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<Outline>,
    /// The component a `component` node stands for (issue 113); its own operation override is in `op`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<ComponentId>,
    #[serde(default, skip_serializing_if = "Params::is_empty")]
    pub params: Params,
    /// Push/pull's and the round tool's edits kept on the node (issues 73 and 88), in the order they apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<ObjectEdit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeData>,
}

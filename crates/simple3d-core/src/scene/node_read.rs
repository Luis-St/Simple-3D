//! Reading a node: what kind it is, and the parameters behind it.

use super::*;
use crate::mesh_data::MeshData;
use crate::primitive::{self, Params, PrimitiveSpec};
use simple3d_geom::tiling::SplitPlan;
use simple3d_geom::Vec3;
use std::sync::Arc;

impl Node {
    /// The smallest a scale factor may get. Zero collapses a solid into a plane
    /// and negative turns it inside out, and neither is a thing to export.
    pub const MIN_SCALE: f64 = 1e-4;

    /// The node's own visibility, as the one three-state answer the interface
    /// asks for rather than as the two flags that store it.
    pub fn visibility(&self) -> Visibility {
        match (self.visible, self.ghost) {
            (true, _) => Visibility::Visible,
            (false, true) => Visibility::Ghost,
            (false, false) => Visibility::Hidden,
        }
    }

    pub fn set_visibility(&mut self, visibility: Visibility) {
        self.visible = visibility == Visibility::Visible;
        self.ghost = visibility == Visibility::Ghost;
    }

    /// A scale with every axis clamped into the range that produces a solid.
    pub fn sane_scale(scale: Vec3) -> Vec3 {
        Vec3::new(scale.x.max(Node::MIN_SCALE), scale.y.max(Node::MIN_SCALE), scale.z.max(Node::MIN_SCALE))
    }

    pub fn is_group(&self) -> bool {
        matches!(self.body, Body::Group { .. })
    }

    pub fn is_pattern(&self) -> bool {
        matches!(self.body, Body::Pattern { .. })
    }

    pub fn is_mesh(&self) -> bool {
        matches!(self.body, Body::Mesh { .. })
    }

    /// Whether this node is a shape that was broken into its pieces (issue 82).
    pub fn is_split(&self) -> bool {
        matches!(self.body, Body::Split { .. })
    }

    /// The object a split was made from, for a node that is one.
    pub fn split_original(&self) -> Option<&Arc<NodeData>> {
        match &self.body {
            Body::Split { original, .. } => Some(original),
            _ => None,
        }
    }

    /// How a split's pieces were cut, for one cut into a pattern of cells.
    /// `None` for a node that is not a split, and for a split that was
    /// separated into the pieces it was already in rather than cut.
    pub fn split_plan(&self) -> Option<&SplitPlan> {
        match &self.body {
            Body::Split { plan, .. } => plan.as_ref(),
            _ => None,
        }
    }

    /// The geometry this node owns, for a body that owns any.
    pub fn mesh(&self) -> Option<&Arc<MeshData>> {
        match &self.body {
            Body::Mesh { mesh } => Some(mesh),
            _ => None,
        }
    }

    /// What kind of thing this node is, in one word, for a status line or a
    /// tooltip. Not the primitive's own label -- "Box" -- but the family.
    pub fn kind_label(&self) -> &'static str {
        match &self.body {
            Body::Group { .. } => "group",
            Body::Primitive { .. } => "shape",
            Body::Pattern { .. } => "pattern",
            Body::Mesh { .. } => "mesh",
            Body::Split { .. } => "split",
        }
    }

    /// Whether this node can hold children: a group, a pattern or a split. A
    /// primitive and a mesh cannot, and a drag or an Add that would put a child
    /// under one is refused.
    pub fn can_hold_children(&self) -> bool {
        self.is_group() || self.is_pattern() || self.is_split()
    }

    /// The node's own boolean operation, which only a group has. A split
    /// combines its children too -- see [`Node::combine_op`] -- but it is not a
    /// group, and nothing that edits an operation may reach it.
    pub fn group_op(&self) -> Option<GroupOp> {
        match self.body {
            Body::Group { op } => Some(op),
            _ => None,
        }
    }

    /// How this node's children are combined, for the bodies that combine
    /// children at all: the group's own operation, and a union for a split,
    /// because pieces of one shape standing side by side is what a union is.
    pub fn combine_op(&self) -> Option<GroupOp> {
        match self.body {
            Body::Group { op } => Some(op),
            Body::Split { .. } => Some(GroupOp::Union),
            _ => None,
        }
    }

    pub fn spec(&self) -> Option<&'static PrimitiveSpec> {
        match &self.body {
            Body::Primitive { type_id, .. } => primitive::lookup(type_id),
            _ => None,
        }
    }

    pub fn params(&self) -> Option<&Params> {
        match &self.body {
            Body::Primitive { params, .. } | Body::Pattern { params } => Some(params),
            _ => None,
        }
    }

    pub fn params_mut(&mut self) -> Option<&mut Params> {
        match &mut self.body {
            Body::Primitive { params, .. } | Body::Pattern { params } => Some(params),
            _ => None,
        }
    }
}

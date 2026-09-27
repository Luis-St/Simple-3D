//! Reading a node: what kind it is, and the parameters behind it.

use super::*;
use crate::mesh_data::MeshData;
use crate::primitive::{self, Params, PrimitiveSpec};
use simple3d_geom::tiling::SplitPlan;
use simple3d_geom::Vec3;
use std::sync::Arc;

impl Node {
    /// The smallest scale factor: zero flattens and negative inverts the solid.
    pub const MIN_SCALE: f64 = 1e-4;

    /// The node's visibility as one three-state answer rather than two flags.
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

    /// Whether this node is a shape broken into pieces (issue 82).
    pub fn is_split(&self) -> bool {
        matches!(self.body, Body::Split { .. })
    }

    /// Whether this node is an integration of a component (issue 113).
    pub fn is_component(&self) -> bool {
        matches!(self.body, Body::Component { .. })
    }

    /// The component this node stands for, when it is an integration.
    pub fn component(&self) -> Option<ComponentId> {
        match self.body {
            Body::Component { component, .. } => Some(component),
            _ => None,
        }
    }

    /// The operation an integration uses instead of its component's, if chosen.
    pub fn component_op(&self) -> Option<GroupOp> {
        match self.body {
            Body::Component { op, .. } => op,
            _ => None,
        }
    }

    /// The object a split was made from.
    pub fn split_original(&self) -> Option<&Arc<NodeData>> {
        match &self.body {
            Body::Split { original, .. } => Some(original),
            _ => None,
        }
    }

    /// How a split's pieces were cut; `None` for non-splits and plain separations.
    pub fn split_plan(&self) -> Option<&SplitPlan> {
        match &self.body {
            Body::Split { plan, .. } => plan.as_ref(),
            _ => None,
        }
    }

    /// The geometry this node owns, if any.
    pub fn mesh(&self) -> Option<&Arc<MeshData>> {
        match &self.body {
            Body::Mesh { mesh } => Some(mesh),
            _ => None,
        }
    }

    /// The node's family in one word ("Group", not the primitive's own label), for status and tooltips.
    pub fn kind_label(&self) -> &'static str {
        match &self.body {
            Body::Group { .. } => "group",
            Body::Primitive { .. } => "shape",
            Body::Pattern { .. } => "pattern",
            Body::Mesh { .. } => "mesh",
            Body::Split { .. } => "split",
            Body::Component { .. } => "component",
        }
    }

    /// Whether this node can hold children: a group, pattern or split.
    pub fn can_hold_children(&self) -> bool {
        self.is_group() || self.is_pattern() || self.is_split()
    }

    /// The node's own boolean operation, groups only; splits combine too ([`Node::combine_op`]) but
    /// are not editable as groups.
    pub fn group_op(&self) -> Option<GroupOp> {
        match self.body {
            Body::Group { op } => Some(op),
            _ => None,
        }
    }

    /// How this node combines its children: a group's operation, or a union for a split.
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

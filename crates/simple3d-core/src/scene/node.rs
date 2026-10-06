//! A node of the scene tree, and whether it is shown.

use super::*;
use simple3d_geom::Vec3;

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    /// Millimetres, in the parent's frame.
    pub position: Vec3,
    /// Degrees, applied X then Y then Z.
    pub rotation: Vec3,
    /// A per-axis factor in the node's own axes, before rotation. Unlike a resize it is carried by the
    /// node and works on groups. `1, 1, 1` is no scale.
    pub scale: Vec3,
    pub anchor: Anchor,
    /// Hidden nodes are excluded from evaluation and export entirely.
    pub visible: bool,
    /// How a hidden node looks: nothing, or a translucent ghost (for positioning a tool body). Read via
    /// `Node::visibility`.
    pub ghost: bool,
    /// This node's paint; without one it takes its nearest painted ancestor's colour.
    pub colour: Option<Colour>,
    /// Per-object override of the scene's default segment count.
    pub segments: Option<u32>,
    /// This node's role in a chosen-bodies export; `None` means a body of its own.
    pub export_body: Option<ExportBody>,
    /// Whether this piece has been lifted out of its split to get its own tree row (issue 82). Read
    /// via [`Scene::has_row`], since the answer depends on the parent too.
    pub extracted: bool,
    pub body: Body,
    /// Push/pull's additions and cuts and the round tool's treated edges, applied to the body's result
    /// in order (issues 73 and 88).
    pub edits: Vec<ObjectEdit>,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
}

impl Node {
    /// A node at the origin with default properties, holding `body`.
    pub(crate) fn fresh(id: NodeId, name: String, body: Body, parent: Option<NodeId>) -> Node {
        Node {
            id,
            name,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body,
            edits: Vec::new(),
            children: Vec::new(),
            parent,
        }
    }
}

/// What a node shows in the viewport: the three states the interface offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Visible,
    /// Excluded from the model and drawn as a translucent shell, for positioning a subtraction.
    Ghost,
    /// Excluded from the model and not drawn at all.
    Hidden,
}

impl Visibility {
    pub const ALL: [Visibility; 3] = [Visibility::Visible, Visibility::Ghost, Visibility::Hidden];

    pub fn label(self) -> &'static str {
        match self {
            Visibility::Visible => "Visible",
            Visibility::Ghost => "Ghost",
            Visibility::Hidden => "Hidden",
        }
    }
}

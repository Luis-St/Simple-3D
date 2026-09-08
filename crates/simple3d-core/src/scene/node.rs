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
    /// A factor per axis, applied in the node's own axes before its rotation.
    ///
    /// Distinct from a *resize*, which rewrites the dimension a shape is defined
    /// by and leaves nothing behind. A scale is a factor the node carries, it
    /// applies to a whole group as readily as to one shape, and it is the only
    /// way to make something a proportion of what it was without touching every
    /// number underneath. `1, 1, 1` is no scale at all.
    pub scale: Vec3,
    pub anchor: Anchor,
    /// Hidden nodes are excluded from evaluation and export entirely.
    pub visible: bool,
    /// What a hidden node looks like: nothing at all, or a translucent ghost.
    ///
    /// Meaningless while `visible` is true, and the pair is read through
    /// `Node::visibility` rather than field by field. It is per node because
    /// the two reasons to hide something are different reasons: a tool body
    /// about to be subtracted has to be seen while it is positioned, and
    /// everything else that is hidden has to be *gone*. One switch over the
    /// whole document could only ever answer one of them.
    pub ghost: bool,
    /// What this node is painted, if anything. A node without one takes its
    /// nearest painted ancestor's colour, which is what makes painting a group
    /// paint everything in it; nothing painted anywhere leaves the theme's own
    /// colour for solids.
    pub colour: Option<Colour>,
    /// Per-object override of the scene's default segment count.
    pub segments: Option<u32>,
    /// What this node is in an export whose bodies the user chooses. `None`,
    /// which is nearly always, means a body of its own.
    pub export_body: Option<ExportBody>,
    /// Whether this node has been lifted out of the collection holding it, so
    /// that it has a row of its own in the tree (issue 82).
    ///
    /// Meaningless everywhere but under a [`Body::Split`], which holds its
    /// pieces *inside* itself: a split is one row however many thousand pieces
    /// it is in, and the ones marked here are the few that have been asked for
    /// by name. Read through [`Scene::has_row`] rather than field by field,
    /// because "is this drawn in the tree" is a question about the node *and*
    /// its parent and answering half of it is how a piece ends up in two places
    /// at once.
    pub extracted: bool,
    pub body: Body,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
}

/// What a node shows in the viewport: the three states the interface offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Visible,
    /// Excluded from the model, drawn as a translucent shell -- for a body that
    /// is about to be subtracted and has to be positioned first.
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

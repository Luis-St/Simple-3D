//! What an outliner drag is carrying, and where it would land.

use simple3d_core::scene::NodeId;

/// What an outliner drag holds. Palette shapes are not in the scene until dropped, so they are a
/// separate case rather than an empty load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carried {
    /// The grabbed row; `dragged_nodes` carries the whole selection if the row is part of it.
    Rows(NodeId),
    /// A palette primitive dropped into the tree instead of added at the insertion point.
    Shape(&'static str),
}

/// Where an outliner drag would drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropTarget {
    pub parent: NodeId,
    pub index: usize,
    /// Set when dropping into a group rather than between siblings, so the indicator differs.
    pub into: Option<NodeId>,
}

//! What an outliner drag is carrying, and where it would land.

use simple3d_core::scene::NodeId;

/// What a drag over the outliner is holding.
///
/// Rows already in the tree are moved by it; a shape from the palette is not in
/// the scene at all until the drop lands, so the two are told apart here rather
/// than by whether the load happens to be empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carried {
    /// The row that was grabbed. What travels with it is `dragged_nodes`: the
    /// whole selection when the grabbed row is part of it, that row alone
    /// otherwise.
    Rows(NodeId),
    /// A primitive type from the palette, dropped into the tree rather than
    /// added at the document's insertion point.
    Shape(&'static str),
}

/// Where an outliner drag would drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropTarget {
    pub parent: NodeId,
    pub index: usize,
    /// Set when the drop is *into* a group rather than between two siblings, so
    /// the indicator can differ.
    pub into: Option<NodeId>,
}

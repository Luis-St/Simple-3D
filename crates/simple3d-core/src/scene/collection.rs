//! Collections: the rows of pieces a split produced, and taking pieces out
//! of one or giving them back.

use super::*;
use crate::mesh_data::MeshData;
use std::sync::Arc;

impl Scene {
    // -- collections (issue 82) ---------------------------------------------

    /// Whether `id` holds its children *inside* itself rather than as rows of
    /// the tree: a split, which is one object in the outliner however many
    /// thousand pieces it is in.
    ///
    /// This is the whole of what makes a cut into ten thousand cells usable.
    /// The pieces are real nodes -- they evaluate, export, take a colour and a
    /// transform apiece -- but a tree with ten thousand rows in it is a tree
    /// nobody can find anything in, so they are reached through the split's own
    /// panel and only the ones asked for by name get a row.
    pub fn is_collection(&self, id: NodeId) -> bool {
        self.nodes.get(&id).is_some_and(Node::is_split)
    }

    /// Whether `id` is drawn as a row of the outliner at all.
    ///
    /// Everything is, except a piece still inside a collection. Asked of the
    /// node *and* its parent, because a piece extracted from one collection and
    /// dragged into another is a piece of the second one now.
    pub fn has_row(&self, id: NodeId) -> bool {
        let Some(node) = self.nodes.get(&id) else { return false };
        match node.parent {
            Some(parent) => node.extracted || !self.is_collection(parent),
            None => true,
        }
    }

    /// The nearest ancestor of `id` that the tree actually draws -- `id` itself
    /// for all but a piece inside a collection, and the collection for one of
    /// those.
    ///
    /// What a click in the viewport means: a click on a piece that has no row
    /// is a click on the collection, the way a click anywhere on a pattern's
    /// output means the pattern. A piece that *has* been extracted is a thing in
    /// its own right and answers as itself.
    pub fn row_for(&self, id: NodeId) -> NodeId {
        let mut walk = id;
        while !self.has_row(walk) {
            match self.nodes.get(&walk).and_then(|node| node.parent) {
                Some(parent) => walk = parent,
                None => break,
            }
        }
        walk
    }

    /// The children of `id` that the tree draws under it: all of them for
    /// everything but a collection, and the extracted ones for a collection.
    pub fn row_children(&self, id: NodeId) -> Vec<NodeId> {
        let Some(node) = self.nodes.get(&id) else { return Vec::new() };
        if !self.is_collection(id) {
            return node.children.clone();
        }
        node.children.iter().copied().filter(|c| self.nodes[c].extracted).collect()
    }

    /// Lift pieces out of the collection holding them, so each gets a row of
    /// its own under it (issue 82). Returns how many were newly marked.
    ///
    /// Only a child of `collection` can be extracted from it, and a piece
    /// already extracted is left alone rather than counted twice.
    pub fn extract_pieces(&mut self, collection: NodeId, pieces: &[NodeId]) -> usize {
        if !self.is_collection(collection) {
            return 0;
        }
        let mine: Vec<NodeId> =
            self.nodes[&collection].children.iter().copied().filter(|c| pieces.contains(c)).collect();
        let mut marked = 0;
        for id in mine {
            let node = self.nodes.get_mut(&id).expect("it was just read out of the collection");
            if !node.extracted {
                node.extracted = true;
                marked += 1;
            }
        }
        marked
    }

    /// Put extracted pieces back inside the collection they came from, losing
    /// their rows again. The inverse of [`Scene::extract_pieces`].
    pub fn return_pieces(&mut self, collection: NodeId, pieces: &[NodeId]) -> usize {
        if !self.is_collection(collection) {
            return 0;
        }
        let mine: Vec<NodeId> =
            self.nodes[&collection].children.iter().copied().filter(|c| pieces.contains(c)).collect();
        let mut cleared = 0;
        for id in mine {
            let node = self.nodes.get_mut(&id).expect("it was just read out of the collection");
            if node.extracted {
                node.extracted = false;
                cleared += 1;
            }
        }
        cleared
    }

    /// Turn a collection into an ordinary union group, its pieces becoming
    /// plain children of it (issue 82).
    ///
    /// What extracting the last piece comes to: with nothing left inside it, a
    /// collection is a container holding a list of objects, which is what a
    /// union group is -- and a union group is the thing the rest of the
    /// application already knows how to edit. The recipe the split was holding
    /// goes with it, so this is where the break stops being reversible; the
    /// caller is the one that says so before doing it.
    ///
    /// Returns false for a node that is not a collection.
    pub fn dissolve_collection(&mut self, id: NodeId) -> bool {
        if !self.is_collection(id) {
            return false;
        }
        for child in self.nodes[&id].children.clone() {
            if let Some(node) = self.nodes.get_mut(&child) {
                node.extracted = false;
            }
        }
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        node.body = Body::Group { op: GroupOp::Union };
        true
    }

    /// Replace a node's body with stored geometry, keeping everything about the
    /// node that is not its shape -- its name, its place in the tree, its
    /// transform, its colour, its export mark (issue 80).
    ///
    /// Its children go with the old body, because they *were* the old body: the
    /// operands of a boolean are not parts of the result, and leaving them in
    /// the tree under a mesh that already contains them would double every
    /// solid in the export.
    pub fn convert_to_mesh(&mut self, id: NodeId, mesh: MeshData) -> bool {
        if id == self.root || !self.nodes.contains_key(&id) {
            return false;
        }
        for child in self.descendants(id) {
            self.nodes.remove(&child);
        }
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        node.children.clear();
        node.body = Body::Mesh { mesh: Arc::new(mesh) };
        true
    }
}

//! Collections: the pieces a split produced, and extracting or returning them.

use super::*;
use crate::mesh_data::MeshData;
use std::sync::Arc;

impl Scene {
    // -- collections (issue 82) ---------------------------------------------

    /// Whether `id` holds its children inside itself rather than as tree rows: a split, one outliner
    /// row however many pieces it has. Pieces are real nodes, reached through the split's panel, and
    /// only extracted ones get rows.
    pub fn is_collection(&self, id: NodeId) -> bool {
        self.nodes.get(&id).is_some_and(Node::is_split)
    }

    /// Whether `id` is an outliner row: everything but a piece still inside a collection. The parent
    /// is checked too, since an extracted piece may have been moved into another collection.
    pub fn has_row(&self, id: NodeId) -> bool {
        let Some(node) = self.nodes.get(&id) else { return false };
        match node.parent {
            Some(parent) => node.extracted || !self.is_collection(parent),
            None => true,
        }
    }

    /// The nearest ancestor of `id` the tree draws: itself, or its collection for an unextracted
    /// piece. A viewport click on such a piece selects the collection.
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

    /// The children the tree draws under `id`: all, or only the extracted ones for a collection.
    pub fn row_children(&self, id: NodeId) -> Vec<NodeId> {
        let Some(node) = self.nodes.get(&id) else { return Vec::new() };
        if !self.is_collection(id) {
            return node.children.clone();
        }
        node.children.iter().copied().filter(|c| self.nodes[c].extracted).collect()
    }

    /// Give pieces their own rows under their collection (issue 82). Returns how many were newly
    /// marked; only children of `collection` count, and already extracted ones are skipped.
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

    /// Put extracted pieces back inside their collection. The inverse of [`Scene::extract_pieces`].
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

    /// Turn a collection into an ordinary union group of its pieces (issue 82), dropping the split's
    /// recipe, which makes the break irreversible; the caller asks first. False for non-collections.
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

    /// Swap the mesh of a node that already holds one, keeping everything else (issue 106). Used by
    /// the simplify tool's preview as a cheap `Arc` swap; non-meshes are refused, since converting
    /// needs an undo step.
    pub fn set_mesh(&mut self, id: NodeId, mesh: Arc<MeshData>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        if !matches!(node.body, Body::Mesh { .. }) {
            return false;
        }
        node.body = Body::Mesh { mesh };
        true
    }

    /// Replace a node's body with stored geometry, keeping its name, place, transform, colour and
    /// export mark (issue 80). Its children go with the old body, or the export would double them.
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

//! The nodes a split leaves behind, and putting one back as it was.

use super::*;
use simple3d_geom::tiling::SplitPlan;
use std::sync::Arc;

impl Scene {
    /// Add the node that holds what a shape was broken into (issue 82).
    ///
    /// It stands where the shape stood and wears the shape's own properties --
    /// its name, its transform, its anchor, its colour, its export mark -- so
    /// the pieces inside it are exactly where they were and the document still
    /// shows one item called what it was called. `original` is kept whole, and
    /// is what [`Scene::restore_split`] puts back.
    ///
    /// `plan` says how the pieces were made, for a shape cut into a pattern
    /// of cells (issue 82); a shape merely separated into the pieces it was
    /// already in passes `None`.
    ///
    /// The caller adds the pieces as children afterwards; a split with none is
    /// as empty as a group with none, and evaluates to nothing.
    pub fn add_split(&mut self, original: NodeData, plan: Option<SplitPlan>, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            // Verbatim, not put through `unique_name`: the shape this was made
            // from is on its way out of the document, and its name goes to the
            // node standing in its place rather than to a second copy of it.
            name: original.name.clone(),
            position: original.position,
            rotation: original.rotation,
            scale: Node::sane_scale(original.scale),
            anchor: original.anchor,
            visible: original.visible,
            ghost: original.ghost,
            colour: original.colour.as_deref().and_then(Colour::from_hex),
            segments: original.segments,
            export_body: original.export_body,
            extracted: original.extracted,
            body: Body::Split { original: Arc::new(original), plan },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }

    /// Put a split back together: the object it was made from returns to its
    /// place, and the pieces go with the split node (issue 82).
    ///
    /// The restored object takes the *split's* current properties, not the ones
    /// it had when it was broken -- moving the pieces about as one item and then
    /// joining them back must not send the shape back to where it started. What
    /// was done to the pieces themselves is not carried across, because the
    /// object coming back is the recipe, not the triangles: that is the whole of
    /// what makes the break reversible, and undo is there for the other answer.
    ///
    /// Returns the id of the restored object, or `None` for a node that is not
    /// a split, or one whose stored recipe this build cannot rebuild.
    pub fn restore_split(&mut self, id: NodeId) -> Option<NodeId> {
        // The whole node, cloned: a split's body is a pointer to its recipe, so
        // this costs nothing and keeps every property in one place.
        let kept = self.nodes.get(&id)?.clone();
        let original = Arc::clone(kept.split_original()?);
        let parent = kept.parent?;
        let index = self.nodes.get(&parent)?.children.iter().position(|&c| c == id)?;
        // Rebuilt before the split is taken out, so a recipe that cannot be
        // rebuilt leaves the document exactly as it was rather than empty-handed.
        let restored = self.import_subtree(&original, parent, index)?;
        self.remove(id);
        let node = self.nodes.get_mut(&restored)?;
        node.name = kept.name;
        node.position = kept.position;
        node.rotation = kept.rotation;
        node.scale = kept.scale;
        node.anchor = kept.anchor;
        node.visible = kept.visible;
        node.ghost = kept.ghost;
        node.colour = kept.colour;
        node.segments = kept.segments;
        node.export_body = kept.export_body;
        // Including whether it had a row: a collection extracted out of another
        // collection and then joined back together is still the piece that was
        // extracted (issue 82).
        node.extracted = kept.extracted;
        self.rename_subtree_uniquely(restored, true);
        Some(restored)
    }
}

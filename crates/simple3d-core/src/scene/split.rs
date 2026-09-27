//! The nodes a split leaves behind, and putting one back as it was.

use super::*;
use simple3d_geom::tiling::SplitPlan;
use std::sync::Arc;

impl Scene {
    /// Add the node holding what a shape was broken into (issue 82). It takes the shape's name,
    /// transform, anchor, colour and export mark, and keeps `original` for [`Scene::restore_split`].
    /// `plan` records a cell pattern, or `None` for a plain separation. The caller adds the pieces.
    pub fn add_split(&mut self, original: NodeData, plan: Option<SplitPlan>, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            // Not made unique: the source shape is leaving, and its name goes to this node.
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

    /// Put a split back together (issue 82): the original returns with the split's current
    /// properties, so moving the pieces as one is kept; edits to individual pieces are not. Returns
    /// the restored id, or `None` for a non-split or an unrebuildable recipe.
    pub fn restore_split(&mut self, id: NodeId) -> Option<NodeId> {
        // Cloned whole: the body is a pointer to the recipe, so this is cheap.
        let kept = self.nodes.get(&id)?.clone();
        let original = Arc::clone(kept.split_original()?);
        let parent = kept.parent?;
        let index = self.nodes.get(&parent)?.children.iter().position(|&c| c == id)?;
        // Rebuilt before the split is removed, so failure leaves the document unchanged.
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
        // Including its extracted mark, so a joined piece stays the extracted piece (issue 82).
        node.extracted = kept.extracted;
        self.rename_subtree_uniquely(restored, true);
        Some(restored)
    }
}

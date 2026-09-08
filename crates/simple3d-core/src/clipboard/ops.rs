//! Copying a selection out and putting it back somewhere else.

use super::*;
use crate::scene::{NodeId, Scene};

/// Copy the topmost nodes of a selection, in tree order. Selecting a group and
/// one of its children copies the group only -- otherwise the child would arrive
/// twice.
pub fn copy(scene: &Scene, selection: &[NodeId]) -> Option<Clip> {
    let order = scene.depth_first();
    let mut tops: Vec<NodeId> = selection
        .iter()
        .copied()
        .filter(|&id| id != scene.root() && scene.contains(id))
        .filter(|&id| !selection.iter().any(|&other| other != id && scene.is_ancestor_of(other, id)))
        .collect();
    tops.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
    tops.dedup();
    if tops.is_empty() {
        return None;
    }
    Some(Clip { format: CLIP_VERSION, nodes: tops.iter().filter_map(|&id| scene.export_subtree(id)).collect() })
}

/// Paste a clip, following the same target rule as Add: into the selected group,
/// or as a sibling of the selected leaf. Returns the new nodes, which the caller
/// leaves selected so a nudge or a drag can follow immediately.
pub fn paste(scene: &mut Scene, clip: &Clip, selection: Option<NodeId>) -> Vec<NodeId> {
    insert(scene, clip, selection, true)
}

/// Paste, saying whether the arriving node is a *copy* of something already
/// here. It is for the clipboard, and it is not for a saved primitive dropped in
/// from the library: that is not a copy of anything in this project, and calling
/// it "Bracket copy" would be a lie the user then has to correct.
pub fn insert(scene: &mut Scene, clip: &Clip, selection: Option<NodeId>, as_copy: bool) -> Vec<NodeId> {
    let (parent, index) = scene.insertion_point(selection);
    let mut created = Vec::new();
    for (offset, data) in clip.nodes.iter().enumerate() {
        let mut data = data.clone();
        data.name = unique_name(scene, &data.name, as_copy);
        if let Some(id) = scene.import_subtree(&data, parent, index + offset) {
            scene.rename_subtree_uniquely(id, true);
            created.push(id);
        }
    }
    created
}

/// `Plate` -> `Plate copy` -> `Plate copy 2`, or `Bracket` -> `Bracket 2` when
/// the node is not a copy of anything. Measured against every name in the
/// document, not just the new parent's children, because the outliner shows
/// every depth at once; the descendants that arrive with it are put through the
/// same rule by `rename_subtree_uniquely`.
pub(crate) fn unique_name(scene: &Scene, name: &str, as_copy: bool) -> String {
    let base = if as_copy { crate::scene::copy_name(name) } else { name.to_string() };
    crate::scene::free_name(&scene.taken_names(), &base)
}

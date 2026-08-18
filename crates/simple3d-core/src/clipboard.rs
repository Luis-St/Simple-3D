//! Copy, cut and paste of whole subtrees (spec section 8.1).
//!
//! The payload is text in the same schema as the project file, so a selection
//! can be pasted into a text editor and back again. It is held in the
//! application's own clipboard rather than the system one -- the spec does not
//! require exchanging with other applications -- but the text form means doing
//! so later is a matter of handing this string to the platform.
//!
//! Pasting into the same parent applies **no offset**: the copy lands exactly on
//! the original. That is deliberate -- it is what makes copy, move, repeat work.
//! Only the name gets a suffix, so the outliner stays readable.

use crate::scene::{NodeData, NodeId, Scene};
use serde::{Deserialize, Serialize};

pub const CLIP_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Clip {
    pub format: u32,
    /// A multi-selection copies as a set and pastes as a set, preserving
    /// relative positions and original order.
    pub nodes: Vec<NodeData>,
}

impl Clip {
    pub fn to_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("a clip always serialises");
        text.push('\n');
        text
    }

    pub fn from_text(text: &str) -> Option<Clip> {
        let clip: Clip = serde_json::from_str(text).ok()?;
        if clip.format > CLIP_VERSION || clip.nodes.is_empty() {
            return None;
        }
        Some(clip)
    }
}

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
        data.name = unique_name(scene, parent, &data.name, as_copy);
        if let Some(id) = scene.import_subtree(&data, parent, index + offset) {
            created.push(id);
        }
    }
    created
}

/// `Plate` -> `Plate copy` -> `Plate copy 2`, or `Bracket` -> `Bracket 2` when
/// the node is not a copy of anything. Only the arriving node's own name is
/// touched; descendants keep theirs, since they are unambiguous inside their
/// parent.
fn unique_name(scene: &Scene, parent: NodeId, name: &str, as_copy: bool) -> String {
    let taken: Vec<&str> = scene.node(parent).children.iter().map(|c| scene.node(*c).name.as_str()).collect();
    let base = if as_copy { format!("{name} copy") } else { name.to_string() };
    if !taken.contains(&base.as_str()) {
        return base;
    }
    for n in 2.. {
        let candidate = format!("{base} {n}");
        if !taken.contains(&candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::GroupOp;
    use simple3d_geom::Vec3;

    fn boxed(scene: &mut Scene, parent: NodeId, x: f64) -> NodeId {
        let index = scene.node(parent).children.len();
        let id = scene.add_primitive("box", parent, index).unwrap();
        scene.get_mut(id).unwrap().position = Vec3::new(x, 0.0, 0.0);
        id
    }

    #[test]
    fn a_scale_travels_with_a_copied_subtree() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let child = scene.add_primitive("box", group, 0).unwrap();
        scene.get_mut(group).unwrap().scale = Vec3::new(2.0, 1.0, 0.5);
        scene.get_mut(child).unwrap().scale = Vec3::new(1.0, 3.0, 1.0);

        let clip = copy(&scene, &[group]).unwrap();
        let pasted = paste(&mut scene, &clip, Some(group))[0];
        assert_eq!(scene.node(pasted).scale, Vec3::new(2.0, 1.0, 0.5));
        let pasted_child = scene.node(pasted).children[0];
        assert_eq!(scene.node(pasted_child).scale, Vec3::new(1.0, 3.0, 1.0));
    }

    #[test]
    fn a_pasted_copy_lands_exactly_on_the_original() {
        // Spec acceptance criterion 20.
        let mut scene = Scene::new();
        let root = scene.root();
        let original = boxed(&mut scene, root, 12.0);
        scene.get_mut(original).unwrap().segments = Some(48);

        let clip = copy(&scene, &[original]).unwrap();
        let pasted = paste(&mut scene, &clip, Some(original));
        assert_eq!(pasted.len(), 1);
        let copy_id = pasted[0];

        assert_ne!(copy_id, original);
        assert_eq!(scene.node(copy_id).position, scene.node(original).position);
        assert_eq!(scene.node(copy_id).segments, Some(48));
        assert_eq!(scene.node(copy_id).name, "Box copy");
        // Directly after the original, as a sibling.
        assert_eq!(scene.node(root).children, vec![original, copy_id]);

        // Editing the copy leaves the original untouched.
        scene.get_mut(copy_id).unwrap().position = Vec3::new(99.0, 0.0, 0.0);
        assert_eq!(scene.node(original).position, Vec3::new(12.0, 0.0, 0.0));
    }

    #[test]
    fn a_nested_subtree_arrives_with_order_and_relative_positions_intact() {
        // Spec acceptance criterion 21.
        let mut scene = Scene::new();
        let root = scene.root();
        let source = scene.add_group(GroupOp::Difference, root, 0);
        let children: Vec<NodeId> = (0..3).map(|i| boxed(&mut scene, source, i as f64 * 7.0)).collect();
        let target = scene.add_group(GroupOp::Union, root, 1);

        let clip = copy(&scene, &[source]).unwrap();
        let pasted = paste(&mut scene, &clip, Some(target))[0];

        assert_eq!(scene.node(pasted).parent, Some(target));
        assert_eq!(scene.node(pasted).group_op(), Some(GroupOp::Difference));
        let arrived = scene.node(pasted).children.clone();
        assert_eq!(arrived.len(), 3);
        for (new, old) in arrived.iter().zip(children.iter()) {
            assert_eq!(scene.node(*new).position, scene.node(*old).position);
            assert_eq!(scene.node(*new).name, scene.node(*old).name);
            assert_ne!(new, old);
        }
    }

    #[test]
    fn a_multi_selection_copies_and_pastes_as_a_set() {
        let mut scene = Scene::new();
        let root = scene.root();
        let ids: Vec<NodeId> = (0..3).map(|i| boxed(&mut scene, root, i as f64 * 10.0)).collect();
        let target = scene.add_group(GroupOp::Union, root, 3);

        let clip = copy(&scene, &ids).unwrap();
        let pasted = paste(&mut scene, &clip, Some(target));
        assert_eq!(pasted.len(), 3);
        assert_eq!(scene.node(target).children, pasted);
        for (new, old) in pasted.iter().zip(ids.iter()) {
            assert_eq!(scene.node(*new).position, scene.node(*old).position);
        }
    }

    #[test]
    fn copying_a_group_and_its_child_does_not_duplicate_the_child() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let child = boxed(&mut scene, group, 0.0);
        let clip = copy(&scene, &[group, child]).unwrap();
        assert_eq!(clip.nodes.len(), 1);
        assert_eq!(clip.nodes[0].children.len(), 1);
    }

    #[test]
    fn the_root_cannot_be_copied() {
        let mut scene = Scene::new();
        let root = scene.root();
        boxed(&mut scene, root, 0.0);
        assert!(copy(&scene, &[root]).is_none());
    }

    #[test]
    fn the_payload_is_text_in_the_project_schema() {
        // Spec section 8.1: paste into a text editor and back again.
        let mut scene = Scene::new();
        let root = scene.root();
        let id = boxed(&mut scene, root, 3.0);
        let clip = copy(&scene, &[id]).unwrap();
        let text = clip.to_text();
        assert!(text.contains("\"type\": \"box\""));
        assert!(text.contains("\"position\""));

        let back = Clip::from_text(&text).expect("re-read the text form");
        let pasted = paste(&mut scene, &back, Some(root))[0];
        assert_eq!(scene.node(pasted).position, Vec3::new(3.0, 0.0, 0.0));
    }

    #[test]
    fn garbage_text_is_not_accepted_as_a_clip() {
        for text in ["", "{}", "[]", "not json", "{\"format\": 99, \"nodes\": []}", "{\"format\":1,\"nodes\":[]}"] {
            assert!(Clip::from_text(text).is_none(), "{text:?}");
        }
    }

    #[test]
    fn repeated_pastes_into_one_parent_get_distinct_names() {
        let mut scene = Scene::new();
        let root = scene.root();
        let id = boxed(&mut scene, root, 0.0);
        let clip = copy(&scene, &[id]).unwrap();
        let first = paste(&mut scene, &clip, Some(root))[0];
        let second = paste(&mut scene, &clip, Some(root))[0];
        assert_eq!(scene.node(first).name, "Box copy");
        assert_eq!(scene.node(second).name, "Box copy 2");
    }
}

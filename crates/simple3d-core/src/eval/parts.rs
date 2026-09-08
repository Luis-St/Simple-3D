//! The separate bodies an export writes, and what each is called.

use super::*;
use crate::scene::{ExportBody, NodeId, Scene};
use crate::xform::Xform;
use simple3d_geom::Mesh;
use std::collections::BTreeMap;

/// One evaluated body per node, rather than the single union [`selection_mesh`]
/// makes of them -- what an export that writes each object as its own component
/// needs (issue 58).
///
/// The bodies are the ones the nodes stand for: a boolean group is the shape it
/// evaluates to, not its operands, exactly as in [`selection_mesh`]. Nodes that
/// are hidden, missing, or evaluate to nothing are left out, so the caller never
/// has to write an empty object.
pub struct Part {
    pub id: NodeId,
    pub name: String,
    pub mesh: Mesh,
}

pub fn part_meshes(scene: &Scene, ids: &[NodeId], frames: &BTreeMap<NodeId, Xform>) -> Vec<Part> {
    let mut evaluator = Evaluator::new();
    let cancel = Cancel::new();
    let mut parts = Vec::new();
    for &id in ids {
        if !scene.contains(id) || !scene.node(id).visible {
            continue;
        }
        let Some(parent) = frames.get(&id) else { continue };
        let mesh = apply(parent, &evaluator.subtree(scene, id, &cancel).mesh);
        if mesh.triangle_count() > 0 {
            parts.push(Part { id, name: scene.node(id).name.clone(), mesh });
        }
    }
    parts
}

/// The bodies a user-chosen export writes: one per [`Part`], each already
/// unioned into a single solid (issue 58).
///
/// The marks are read one node at a time, and every one of them is local:
///
/// * no mark, or a mark this walk cannot honour, is a body of that node alone
///   -- so a project nobody has marked comes out exactly as `part_meshes`
///   would, and adding a shape later needs no decision made about it;
/// * [`ExportBody::Split`] on a separable group is not a body at all: its
///   children are considered in its place, which is how an export reaches
///   inside a group;
/// * [`ExportBody::Shared`] merges, into one solid, every node carrying the
///   same number, wherever in the tree they are.
///
/// Bodies come out in the order the tree reaches them, and a merged one is
/// named for the shapes in it, because that name is all a slicer will show.
pub fn body_meshes(scene: &Scene, ids: &[NodeId], frames: &BTreeMap<NodeId, Xform>) -> Vec<Part> {
    let mut roots: Vec<(NodeId, Option<u32>)> = Vec::new();
    for &id in ids {
        collect_bodies(scene, id, &mut roots);
    }

    // Grouped by number, in the order each number is first met, with every
    // unnumbered node a body of its own.
    let mut bodies: Vec<(Option<u32>, Vec<NodeId>)> = Vec::new();
    for (id, key) in roots {
        match key.and_then(|k| bodies.iter_mut().find(|(other, _)| *other == Some(k))) {
            Some((_, members)) => members.push(id),
            None => bodies.push((key, vec![id])),
        }
    }

    bodies
        .into_iter()
        .filter_map(|(_, members)| {
            let mesh = selection_mesh(scene, &members, frames);
            if mesh.triangle_count() == 0 {
                return None;
            }
            let first = *members.first()?;
            Some(Part { id: first, name: body_name(scene, &members), mesh })
        })
        .collect()
}

pub(crate) fn collect_bodies(scene: &Scene, id: NodeId, out: &mut Vec<(NodeId, Option<u32>)>) {
    if !scene.contains(id) || !scene.node(id).visible {
        return;
    }
    match scene.node(id).export_body {
        // Only where the group really can be taken apart. A mark that says
        // otherwise is one the tree has changed under -- a group turned into a
        // difference since it was made -- and the honest reading of it is the
        // one this export can carry out.
        Some(ExportBody::Split) if scene.can_split_for_export(id) => {
            for &child in &scene.node(id).children {
                collect_bodies(scene, child, out);
            }
        }
        Some(ExportBody::Shared(key)) => out.push((id, Some(key))),
        _ => out.push((id, None)),
    }
}

/// What a body is called in the file. One shape lends its own name; several
/// are named for what is in them, because "Body 2" tells a slicer nothing.
pub(crate) fn body_name(scene: &Scene, members: &[NodeId]) -> String {
    let names: Vec<&str> =
        members.iter().filter(|id| scene.contains(**id)).map(|id| scene.node(*id).name.as_str()).collect();
    match names.len() {
        0 => String::new(),
        1 => names[0].to_string(),
        _ => {
            let joined = names.join(" + ");
            // Long enough to name two or three shapes, short enough to read in
            // a slicer's object list.
            if joined.chars().count() <= 64 {
                joined
            } else {
                format!("{} + {} more", names[0], names.len() - 1)
            }
        }
    }
}

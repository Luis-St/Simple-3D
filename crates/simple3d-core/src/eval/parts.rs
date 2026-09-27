//! The separate bodies an export writes, and what each is called.

use super::*;
use crate::scene::{ExportBody, NodeId, Scene};
use crate::xform::Xform;
use simple3d_geom::Mesh;
use std::collections::BTreeMap;

/// One evaluated body per node rather than [`selection_mesh`]'s single union, for per-object
/// exports (issue 58). Booleans are their results; hidden, missing or empty nodes are skipped.
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

/// The bodies a chosen-bodies export writes, one unioned solid per [`Part`] (issue 58).
///
/// * No mark (or an unusable one): a body of that node alone, so unmarked projects match `part_meshes`.
/// * [`ExportBody::Split`] on a separable group: its children are considered instead.
/// * [`ExportBody::Shared`]: every node with the same number merges into one solid.
///
/// Bodies come out in tree order; a merged one is named for its shapes, as slicers show only names.
pub fn body_meshes(scene: &Scene, ids: &[NodeId], frames: &BTreeMap<NodeId, Xform>) -> Vec<Part> {
    let mut roots: Vec<(NodeId, Option<u32>)> = Vec::new();
    for &id in ids {
        collect_bodies(scene, id, &mut roots);
    }

    // Grouped by number in first-seen order; unnumbered nodes are bodies of their own.
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
        // Only where the group really can be split; a stale mark is read as what this export can do.
        Some(ExportBody::Split) if scene.can_split_for_export(id) => {
            for &child in &scene.node(id).children {
                collect_bodies(scene, child, out);
            }
        }
        Some(ExportBody::Shared(key)) => out.push((id, Some(key))),
        _ => out.push((id, None)),
    }
}

/// A body's name in the file: its shape's own, or its shapes' names joined, since "Body 2" says nothing.
pub(crate) fn body_name(scene: &Scene, members: &[NodeId]) -> String {
    let names: Vec<&str> =
        members.iter().filter(|id| scene.contains(**id)).map(|id| scene.node(*id).name.as_str()).collect();
    match names.len() {
        0 => String::new(),
        1 => names[0].to_string(),
        _ => {
            let joined = names.join(" + ");
            // Long enough for two or three names, short enough for a slicer's list.
            if joined.chars().count() <= 64 {
                joined
            } else {
                format!("{} + {} more", names[0], names.len() - 1)
            }
        }
    }
}

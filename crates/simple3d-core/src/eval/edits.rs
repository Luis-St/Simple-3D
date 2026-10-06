//! Push/pull's and the round tool's edits applied to the node holding them (issues 73 and 88).

use super::*;
use crate::scene::{GroupOp, NodeId, ObjectEdit, Placing, Scene};
use simple3d_geom::Mesh;

/// `mesh`, a node's own geometry after its anchor, with its edits applied in order (issues 73 and 88).
/// Each solid takes the object's colour and source, so a face it leaves flush with the object's own
/// is one face, and a rounded surface is the object's to pick. On a group holding several objects,
/// `parts`, their meshes in the same frame, give each face a rounding cuts the colour and source of
/// the object it cuts into instead.
pub(super) fn with_edits(
    scene: &Scene,
    id: NodeId,
    mesh: &Mesh,
    parts: &[std::sync::Arc<Mesh>],
    errors: &mut Vec<NodeError>,
    cancel: &Cancel,
) -> Mesh {
    let node = scene.node(id);
    // A boolean holding an addition stamps none of its own: its base object's is the one whose faces
    // the addition was pushed from and meets.
    let mut stamped = id;
    while !source_stamped(scene, stamped) {
        match scene.node(stamped).children.iter().copied().find(|&child| scene.node(child).visible) {
            Some(child) => stamped = child,
            None => break,
        }
    }
    let tag = match mesh.triangle_count() {
        0 => crate::scene::colour_tag(scene.effective_colour(id)),
        _ => mesh.tag(0),
    };
    let mut result = mesh.clone();
    // Still the union of `parts`, with no edit applied yet.
    let mut pristine = true;
    for edit in &node.edits {
        // Fillets join before cutters cut, as the round tool always placed them.
        let steps = match edit {
            ObjectEdit::Push(edit) => match edit.placing {
                Placing::Add => vec![(GroupOp::Union, vec![edit.solid()])],
                Placing::Cut => vec![(GroupOp::Difference, vec![edit.solid()])],
            },
            ObjectEdit::Round(edit) => {
                let (adds, cuts) = edit.solids(&result);
                vec![(GroupOp::Union, adds), (GroupOp::Difference, cuts)]
            }
        };
        let rounding = matches!(edit, ObjectEdit::Round(_));
        for (op, solids) in steps {
            let mut operands = vec![result];
            for mut solid in solids.into_iter().filter(|solid| solid.triangle_count() > 0) {
                solid.set_tag(tag);
                solid.set_source(source_of(stamped));
                operands.push(solid);
            }
            result = if operands.len() == 1 {
                operands.pop().expect("one operand")
            } else if rounding && op == GroupOp::Difference && pristine && parts.len() > 1 {
                pristine = false;
                cut_each(&operands[1..], parts, id, &node.name, errors, cancel)
            } else {
                pristine = false;
                combine(op, &operands, id, &node.name, errors, cancel).0
            };
            if cancel.is_cancelled() {
                return result;
            }
        }
    }
    result
}

/// The union of `parts` with `cutters` taken out of each part they reach on its own, then joined: the
/// same shape as cutting the union, but each part's walls are its own faces in its own colour, split
/// where the parts meet. Cut out of the union, one wall running from one part into the next took the
/// colour of whichever part's face was nearest.
fn cut_each(
    cutters: &[Mesh],
    parts: &[std::sync::Arc<Mesh>],
    id: NodeId,
    name: &str,
    errors: &mut Vec<NodeError>,
    cancel: &Cancel,
) -> Mesh {
    let reach = cutters.iter().filter_map(Mesh::bounds).reduce(|(lo, hi), (l, h)| (lo.min(l), hi.max(h)));
    let overlaps = |part: &Mesh| match (part.bounds(), reach) {
        (Some((lo, hi)), Some((l, h))) => {
            lo.x <= h.x && l.x <= hi.x && lo.y <= h.y && l.y <= hi.y && lo.z <= h.z && l.z <= hi.z
        }
        _ => false,
    };
    let pieces: Vec<Mesh> = parts
        .iter()
        .map(|part| {
            if !overlaps(part) {
                return (**part).clone();
            }
            let mut operands = vec![(**part).clone()];
            operands.extend(cutters.iter().cloned());
            combine(GroupOp::Difference, &operands, id, name, errors, cancel).0
        })
        .collect();
    combine(GroupOp::Union, &pieces, id, name, errors, cancel).0
}

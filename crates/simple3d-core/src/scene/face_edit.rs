//! Push/pull's changes kept inside the object they were made on (issue 73): a solid swept from a
//! captured face, added to or cut out of the object's shape.
//!
//! Kept on the object rather than as nodes beside it, so the result is one object with one source:
//! the walls of an extrusion flush with the object's own faces merge with them into a single face,
//! which the next push/pull takes whole. An edit can be taken back out as an `extrusion` node of its
//! own, which leaves the model as it was, or dropped, which reverts the object.
//!
//! Which object holds an edit follows what is drawn, as the separate nodes did: the outermost node
//! below the nearest union -- the object itself in a union, or the boolean it is part of. Held by an
//! operand, an addition would be cut, intersected away or hulled, and the walls of a cut carry the
//! base's source, so a pocket's floor pushed up would be filled and cut out again. Cuts go to the same
//! node, so they apply after the additions before them. That is the same as cutting the object
//! itself, since a face always belongs to a base or an intersected operand, never to a cutter, and
//! `(A - C) * B` is `(A * B) - C`.

use super::extrusion::sweep_in;
use super::*;
use crate::primitive::{ParamValue, Params};
use crate::xform::Xform;
use serde::{Deserialize, Serialize};
use simple3d_geom::push_pull::{extrude_outline, Outline};
use simple3d_geom::Mesh;
use std::collections::BTreeMap;
use std::sync::Arc;

/// One push or pull stored on an object.
///
/// The outline lies in the edit's own XY plane and is swept along its +Z by `distance`. The edit's
/// frame is placed in the object's own frame -- after its anchor, before its scale, rotation and
/// position -- so it moves, turns and scales with the object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceEdit {
    pub placing: Placing,
    pub outline: Outline,
    pub distance: f64,
    #[serde(default = "Vec3_zero")]
    pub position: Vec3,
    #[serde(default = "Vec3_zero")]
    pub rotation: Vec3,
}

impl FaceEdit {
    /// The swept solid in the object's own frame. Empty for a distance too small to be a solid.
    pub fn solid(&self) -> Mesh {
        extrude_outline(&self.outline, self.distance).transformed(self.position, self.rotation)
    }

    /// The edit's frame within the object's own frame.
    pub fn frame(&self) -> Xform {
        Xform::from_pos_rot(self.position, self.rotation)
    }

    /// The edit read from a file, or `None` when it could not be a solid.
    pub(crate) fn sane(mut self) -> Option<FaceEdit> {
        let finite = |v: Vec3| v.x.is_finite() && v.y.is_finite() && v.z.is_finite();
        if !self.distance.is_finite() || !finite(self.position) || !finite(self.rotation) {
            return None;
        }
        self.distance = self.distance.max(0.01);
        self.outline = Outline::new(self.outline.outer, self.outline.holes)?;
        Some(self)
    }
}

impl Scene {
    /// Push (`distance > 0`) or pull (`distance < 0`) `face` of `owner` (issue 73), kept as an edit on
    /// `owner` or the boolean it is in (see the module). Returns the node holding it and the edit's
    /// index. `frames` is `Evaluated::node_frames`.
    pub fn push_pull(
        &mut self,
        owner: NodeId,
        face: &CapturedFace,
        distance: f64,
        frames: &BTreeMap<NodeId, Xform>,
    ) -> Option<(NodeId, usize)> {
        if !distance.is_finite() || distance.abs() < 1e-6 || owner == self.root || !self.nodes.contains_key(&owner) {
            return None;
        }
        let placing = if distance > 0.0 { Placing::Add } else { Placing::Cut };
        let holder = self.edit_holder(owner);
        let own = self.own_frame(holder, frames)?;
        // A pull sweeps into the model: its frame is the face's turned over, so +Z points inward, and
        // the outline is mirrored to stay where it was.
        let flip = if distance > 0.0 { 1.0 } else { -1.0 };
        let world = Xform {
            m: [
                [face.u.x, face.v.x * flip, face.normal.x * flip],
                [face.u.y, face.v.y * flip, face.normal.y * flip],
                [face.u.z, face.v.z * flip, face.normal.z * flip],
            ],
            t: face.origin,
        };
        let mirror = |points: &Vec<[f64; 2]>| points.iter().map(|p| [p[0], p[1] * flip]).collect();
        let outline = Outline::new(mirror(&face.outline.outer), face.outline.holes.iter().map(mirror).collect())?;
        let (outline, distance_there, position, rotation) = sweep_in(&own, &world, &outline, distance.abs());
        let edit = FaceEdit { placing, outline, distance: distance_there.max(0.01), position, rotation };
        let edits = &mut self.nodes.get_mut(&holder)?.edits;
        edits.push(ObjectEdit::Push(edit));
        Some((holder, edits.len() - 1))
    }

    /// The node an edit of `owner`'s face is held by: the outermost below a union, an assembly, a
    /// split or the root, whose children are each drawn whole.
    pub fn edit_holder(&self, owner: NodeId) -> NodeId {
        let mut holder = owner;
        while let Some(parent) = self.nodes[&holder].parent {
            let whole = matches!(self.nodes[&parent].body, Body::Split { .. })
                || matches!(self.nodes[&parent].group_op(), Some(GroupOp::Union | GroupOp::Assembly));
            if parent == self.root || whole {
                break;
            }
            holder = parent;
        }
        holder
    }

    /// Take push/pull edit `index` out of `id` as an `extrusion` node of its own (issue 73), placed so
    /// the model is unchanged: an addition joined to the object, a cut taken out of it. Returns the new
    /// node; `None` for an edit that is not a push or pull.
    pub fn extract_face_edit(&mut self, id: NodeId, index: usize, frames: &BTreeMap<NodeId, Xform>) -> Option<NodeId> {
        let edit = self.nodes.get(&id)?.edits.get(index)?.as_push()?.clone();
        let world = self.own_frame(id, frames)?.compose(&edit.frame());
        let suffix = match edit.placing {
            Placing::Add => "extrusion",
            Placing::Cut => "reduction",
        };
        let name = format!("{} {suffix}", self.nodes[&id].name);
        let extracted = self.place_solid(id, edit.placing, true, &name, frames, |parent| {
            let (outline, distance, position, rotation) = sweep_in(parent, &world, &edit.outline, edit.distance);
            let mut params = extrusion_params(&Params::new());
            params.insert("distance".into(), ParamValue::Length(distance.max(0.01)));
            (Body::Extrusion { outline: Arc::new(outline), params }, position, rotation)
        })?;
        self.nodes.get_mut(&id)?.edits.remove(index);
        Some(extracted)
    }

    /// The frame `id`'s own geometry is in -- its parent's frame and its placement, without the anchor
    /// shift -- which its edits are placed in.
    pub(super) fn own_frame(&self, id: NodeId, frames: &BTreeMap<NodeId, Xform>) -> Option<Xform> {
        let node = self.nodes.get(&id)?;
        let placement = Xform::from_pos_rot_scale(node.position, node.rotation, Node::sane_scale(node.scale));
        Some(frames.get(&id)?.compose(&placement))
    }
}

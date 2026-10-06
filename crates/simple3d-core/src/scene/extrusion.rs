//! Solids added beside or cut out of an object of the model: push/pull's edits taken out of their
//! object as `extrusion` nodes (issue 73), and the cutters and fillets of edge rounding (issue 88).
//!
//! Both start from an object found under a face of the evaluated model and must change the model
//! there, whatever booleans the object sits in, so where the new node goes is decided here once:
//!
//! * Material is **added** at the nearest union above the object (or an assembly, or the scene).
//!   Inside a difference, intersection or hull an added solid would be cut, intersected away or
//!   hulled; a sibling of the boolean's result adds exactly what was drawn. An edit taken out of its
//!   object joins that object instead, in a union, so the model does not change.
//! * Material is **taken away** in a difference with the object as its base: the one the object is
//!   already in, or a new one wrapped around it. Later cuts append to that same group rather than
//!   nesting. Any boolean the object is in still applies on top, so `(A - C) * B` cuts the
//!   intersection's result as drawn.

use super::*;
use crate::mesh_data::MeshData;
use crate::primitive::{ParamKind, ParamSpec, ParamValue, Params};
use crate::xform::Xform;
use serde::{Deserialize, Serialize};
use simple3d_geom::push_pull::Outline;
use simple3d_geom::{Mesh, Vec3};
use std::collections::BTreeMap;
use std::sync::Arc;

/// An extrusion's one parameter: how far its outline is swept.
pub const EXTRUSION_PARAMS: [ParamSpec; 1] = [ParamSpec {
    key: "distance",
    label: "Distance",
    kind: ParamKind::Length { min: 0.01 },
    default: ParamValue::Length(10.0),
    lock_group: 0,
    shown_when: None,
}];

/// An extrusion's parameters read from a file: the distance kept when it is a length, else the default.
pub fn extrusion_params(stored: &Params) -> Params {
    EXTRUSION_PARAMS
        .iter()
        .map(|p| {
            let value = match stored.get(p.key) {
                Some(&ParamValue::Length(v)) if v.is_finite() => ParamValue::Length(v.max(0.01)),
                _ => p.default,
            };
            (p.key.to_string(), value)
        })
        .collect()
}

/// Whether a new solid adds to the model or is cut out of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placing {
    Add,
    Cut,
}

/// A face of the model captured for push/pull: its outline in a world frame lying on the face, with
/// `normal = u x v` pointing out of the model.
#[derive(Clone, Debug)]
pub struct CapturedFace {
    pub outline: Outline,
    pub origin: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    pub normal: Vec3,
}

/// `outline`, lying in `world`'s XY plane and swept `distance` along its +Z, re-expressed in `frame`:
/// the outline and distance there, and the position and rotation placing them. Scale in `frame` is
/// taken into the points and distance, so the placement keeps a unit scale, and a scale that is not
/// along the outline's axes skews nothing.
pub(crate) fn sweep_in(frame: &Xform, world: &Xform, outline: &Outline, distance: f64) -> (Outline, f64, Vec3, Vec3) {
    let local = frame.inverse().compose(world);
    let (a, b, sweep) = (local.axis_vector(0), local.axis_vector(1), local.axis_vector(2));
    let x = a.normalized();
    let mut z = a.cross(b).normalized();
    // A mirroring frame turns the plane over; the solid still goes the way it was swept.
    if sweep.dot(z) < 0.0 {
        z = -z;
    }
    let y = z.cross(x);
    let flat = |points: &Vec<[f64; 2]>| {
        points
            .iter()
            .map(|p| {
                let q = a * p[0] + b * p[1];
                [q.dot(x), q.dot(y)]
            })
            .collect()
    };
    let holes = outline.holes.iter().map(flat).collect();
    let outline = Outline::new(flat(&outline.outer), holes).unwrap_or_else(|| outline.clone());
    (outline, distance * sweep.dot(z), local.t, Xform::rotation_of_axes(x, y, z))
}

impl Scene {
    /// Whether any node is an extrusion or holds push/pull edits, which needs format 5 (issue 73).
    pub fn has_extrusion(&self) -> bool {
        self.nodes.values().any(|node| node.is_extrusion() || !node.edits.is_empty())
    }

    /// The node a solid placed for `owner` stands beside: the owner, or the split it is a piece of,
    /// since a split's children are its pieces and nothing else.
    fn placing_host(&self, owner: NodeId) -> NodeId {
        match self.nodes[&owner].parent {
            Some(parent) if self.nodes[&parent].is_split() => parent,
            _ => owner,
        }
    }

    /// Where a solid `placing` for `owner` goes: its parent and index, and the node whose parent frame
    /// that parent's children share. A cut may wrap the host in a new difference first.
    ///
    /// An `exact` addition joins the owner itself, as an edit taken out of it must to leave the model
    /// unchanged: beside it in a union, else in a new union wrapped around it.
    fn placing_slot(&mut self, owner: NodeId, placing: Placing, exact: bool) -> Option<(NodeId, usize, NodeId)> {
        if owner == self.root || !self.nodes.contains_key(&owner) {
            return None;
        }
        let mut host = self.placing_host(owner);
        match placing {
            Placing::Add if exact => {
                let parent = self.nodes[&host].parent?;
                let index = self.nodes[&parent].children.iter().position(|&c| c == host)?;
                if self.nodes[&parent].group_op() == Some(GroupOp::Union) {
                    return Some((parent, index + 1, host));
                }
                let name = self.unique_name(&format!("{} union", self.nodes[&host].name));
                let group = self.insert_fresh(name, Body::Group { op: GroupOp::Union }, parent, index);
                self.unlink(host);
                self.link(host, group, 0);
                Some((group, 1, host))
            }
            Placing::Add => {
                while let Some(parent) = self.nodes[&host].parent {
                    let op = self.nodes[&parent].group_op();
                    if parent == self.root || matches!(op, Some(GroupOp::Union | GroupOp::Assembly)) {
                        break;
                    }
                    host = parent;
                }
                let parent = self.nodes[&host].parent?;
                let index = self.nodes[&parent].children.iter().position(|&c| c == host)? + 1;
                Some((parent, index, host))
            }
            Placing::Cut => {
                let parent = self.nodes[&host].parent?;
                if self.nodes[&parent].group_op() == Some(GroupOp::Difference) {
                    let index = self.nodes[&parent].children.len();
                    return Some((parent, index, host));
                }
                let index = self.nodes[&parent].children.iter().position(|&c| c == host)?;
                let name = self.unique_name(&format!("{} cut", self.nodes[&host].name));
                let group = self.insert_fresh(name, Body::Group { op: GroupOp::Difference }, parent, index);
                self.unlink(host);
                self.link(host, group, 0);
                Some((group, 1, host))
            }
        }
    }

    /// Insert `body` named `name` where a solid `placing` for `owner` goes, its placement set by
    /// `place` from the frame its parent's children are in (`frames` is `Evaluated::node_frames`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn place_solid(
        &mut self,
        owner: NodeId,
        placing: Placing,
        exact: bool,
        name: &str,
        frames: &BTreeMap<NodeId, Xform>,
        place: impl FnOnce(&Xform) -> (Body, Vec3, Vec3),
    ) -> Option<NodeId> {
        // Checked before anything changes, so a refusal leaves the tree as it was.
        frames.get(&self.placing_host(owner))?;
        // The owner's own paint, so the added solid matches the object it grew from.
        let colour = self.effective_colour(owner);
        let (parent, index, beside) = self.placing_slot(owner, placing, exact)?;
        // The new node's siblings, or the wrapped host, already have the frame its parent gives.
        // A difference wrapped earlier in the same edit has no evaluated frame yet; it sits at its
        // parent's origin, so its first child's frame is its own.
        let parent_frame = match frames.get(&beside) {
            Some(frame) => *frame,
            None => *self.nodes[&beside].children.iter().find_map(|child| frames.get(child))?,
        };
        let (body, position, rotation) = place(&parent_frame);
        let name = self.unique_name(name);
        let id = self.insert_fresh(name, body, parent, index);
        let node = self.nodes.get_mut(&id)?;
        node.position = position;
        node.rotation = rotation;
        if placing == Placing::Add {
            node.colour = colour;
        }
        Some(id)
    }

    /// Add `mesh`, given in world space, as a mesh node `placing` for `owner` (issue 88).
    pub fn place_mesh(
        &mut self,
        owner: NodeId,
        placing: Placing,
        name: &str,
        mesh: &Mesh,
        frames: &BTreeMap<NodeId, Xform>,
    ) -> Option<NodeId> {
        self.place_solid(owner, placing, false, name, frames, |parent| {
            let inverse = parent.inverse();
            let mut local = mesh.clone();
            for p in local.positions.iter_mut() {
                *p = inverse.point(*p);
            }
            (Body::Mesh { mesh: Arc::new(MeshData { mesh: local }) }, Vec3::ZERO, Vec3::ZERO)
        })
    }
}

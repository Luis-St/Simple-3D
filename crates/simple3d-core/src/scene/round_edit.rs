//! Rounded and bevelled edges kept on the object they were made on (issue 88), like push/pull's edits.
//!
//! The edit holds the picked edges and corners in the object's own frame and the treatment's size;
//! the cutters and fillets are remade from them whenever the object is evaluated, so the size can
//! still be changed, and the edit can be dropped or taken out as mesh nodes like a push or pull. It
//! is held by the same node a push/pull edit of the same face would be ([`Scene::edit_holder`]).

use super::*;
use crate::xform::Xform;
use serde::{Deserialize, Serialize};
use simple3d_geom::rounding::{
    corner_chamfer, corner_fits, corner_round, edge_ends, edge_fits, edge_room, edge_solid_ends, Corner, FeatureEdge,
    Treatment,
};
use simple3d_geom::{evaluate_boolean, BooleanOp, Mesh};
use std::collections::BTreeMap;

/// What is done to the edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundKind {
    Round,
    Chamfer,
}

impl RoundKind {
    pub const ALL: [RoundKind; 2] = [RoundKind::Round, RoundKind::Chamfer];

    pub fn label(self) -> &'static str {
        match self {
            RoundKind::Round => "Round",
            RoundKind::Chamfer => "Chamfer",
        }
    }
}

fn default_segments() -> u32 {
    8
}

/// One rounding or bevel of some edges and corners of an object, in its own frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoundEdit {
    pub kind: RoundKind,
    /// The arc's radius, or how far back along each face a bevel starts.
    pub size: f64,
    /// How many flat pieces each arc is made of.
    #[serde(default = "default_segments")]
    pub segments: u32,
    pub edges: Vec<FeatureEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corners: Vec<Corner>,
    /// The inside corners picked to mitre the two edges meeting there
    /// ([`simple3d_geom::rounding::inside_corners`]); elsewhere edges end flat.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joints: Vec<Vec3>,
}

impl RoundEdit {
    pub fn treatment(&self) -> Treatment {
        match self.kind {
            RoundKind::Round => Treatment::Round { radius: self.size.max(0.01), segments: self.segments.clamp(1, 64) },
            RoundKind::Chamfer => Treatment::Chamfer { distance: self.size.max(0.01) },
        }
    }

    /// The solids that add to and cut from `solid`, the shape the edit applies to, in its frame:
    /// fillets of the inside edges, cutters of the outside edges and the corners. Edges and corners
    /// without room for the size are left as they are, rather than cut through what lies beyond.
    pub fn solids(&self, solid: &Mesh) -> (Vec<Mesh>, Vec<Mesh>) {
        let treatment = self.treatment();
        let (mut adds, mut cuts) = (Vec::new(), Vec::new());
        let fitting: Vec<FeatureEdge> =
            self.edges.iter().filter(|edge| edge_fits(edge, treatment, edge_room(solid, edge))).copied().collect();
        for (index, edge) in fitting.iter().enumerate() {
            // Two rounds meeting at an inside corner are mitred into one seam.
            let ends = edge_ends(solid, &fitting, index, treatment, &self.joints);
            let Some(mesh) = edge_solid_ends(edge, treatment, ends) else { continue };
            if edge.convex {
                cuts.push(mesh);
            } else {
                adds.push(mesh);
            }
        }
        for corner in self.corners.iter().filter(|corner| corner_fits(corner, treatment)) {
            let mesh = match treatment {
                Treatment::Round { radius, segments } => corner_round(corner, radius, segments),
                Treatment::Chamfer { distance } => corner_chamfer(corner, distance),
            };
            cuts.extend(mesh);
        }
        // One cutter, so two mitred against each other leave no face on the plane they meet at.
        if cuts.len() > 1 {
            cuts = vec![evaluate_boolean(BooleanOp::Union, &cuts)];
        }
        (adds, cuts)
    }

    /// The edit carried by `x`: points moved, normals kept square to their faces, and the size scaled
    /// by the frame's mean scale, which is exact for a uniform one.
    pub fn carried(&self, x: &Xform) -> RoundEdit {
        let inverse = x.inverse();
        let normal = |n: Vec3| {
            Vec3::new(inverse.axis_vector(0).dot(n), inverse.axis_vector(1).dot(n), inverse.axis_vector(2).dot(n))
                .normalized()
        };
        let edges = self
            .edges
            .iter()
            .map(|edge| {
                let (a, b) = (x.point(edge.a), x.point(edge.b));
                let t = (b - a).normalized();
                // Still in its face, but square to the edge again under a scale that is not uniform.
                let along = edge.along.map(|v| {
                    let v = x.vector(v);
                    (v - t * v.dot(t)).normalized()
                });
                FeatureEdge { a, b, normals: edge.normals.map(normal), along, ..*edge }
            })
            .collect();
        let corners = self
            .corners
            .iter()
            .map(|corner| Corner {
                at: x.point(corner.at),
                edges: corner
                    .edges
                    .iter()
                    .map(|&(dir, length)| {
                        let v = x.vector(dir * length);
                        (v.normalized(), v.length())
                    })
                    .collect(),
                faces: corner.faces.iter().map(|&n| normal(n)).collect(),
                source: corner.source,
            })
            .collect();
        let det = x.axis_vector(0).dot(x.axis_vector(1).cross(x.axis_vector(2)));
        let scale = det.abs().cbrt();
        let size = if scale > 1e-12 { self.size * scale } else { self.size };
        let joints = self.joints.iter().map(|&p| x.point(p)).collect();
        RoundEdit { kind: self.kind, size, segments: self.segments, edges, corners, joints }
    }

    /// The edit read from a file, or `None` when it could not be a solid.
    pub(crate) fn sane(mut self) -> Option<RoundEdit> {
        let finite = |v: Vec3| v.x.is_finite() && v.y.is_finite() && v.z.is_finite();
        if !self.size.is_finite() {
            return None;
        }
        self.size = self.size.max(0.01);
        self.segments = self.segments.clamp(1, 64);
        self.edges.retain(|e| finite(e.a) && finite(e.b) && e.normals.iter().chain(&e.along).all(|&v| finite(v)));
        self.corners.retain(|c| finite(c.at) && c.faces.iter().all(|&n| finite(n)));
        self.joints.retain(|&p| finite(p));
        (!self.edges.is_empty() || !self.corners.is_empty()).then_some(self)
    }
}

impl Scene {
    /// The edits that treat `edges` and `corners`, given in world space, each with the object whose
    /// face it was found on: one edit per node holding them ([`Scene::round_holders`]), in that node's
    /// own frame. Not added to
    /// the scene, so the round tool can show them as a draft first.
    pub fn round_edits(
        &self,
        edges: &[(NodeId, FeatureEdge)],
        corners: &[(NodeId, Corner)],
        world: &RoundEdit,
        frames: &BTreeMap<NodeId, Xform>,
    ) -> Vec<(NodeId, RoundEdit)> {
        let mut by_holder: BTreeMap<NodeId, RoundEdit> = BTreeMap::new();
        let empty = || RoundEdit { edges: Vec::new(), corners: Vec::new(), joints: Vec::new(), ..world.clone() };
        let usable = |owner: NodeId| owner != self.root && self.nodes.contains_key(&owner);
        let edges: Vec<(NodeId, FeatureEdge)> = edges.iter().filter(|(o, _)| usable(*o)).cloned().collect();
        let corners: Vec<(NodeId, Corner)> = corners.iter().filter(|(o, _)| usable(*o)).cloned().collect();
        let mut holders = self.round_holders(&edges, &corners, &world.joints).into_iter();
        // The sources name the model's faces, which the edit is not tied to and does not save.
        for (_, edge) in &edges {
            let edge = FeatureEdge { sources: [0; 2], ..*edge };
            let edit = by_holder.entry(holders.next().expect("one per pick")).or_insert_with(empty);
            if !edit.edges.contains(&edge) {
                edit.edges.push(edge);
            }
        }
        for (_, corner) in &corners {
            let corner = Corner { source: 0, ..corner.clone() };
            by_holder.entry(holders.next().expect("one per pick")).or_insert_with(empty).corners.push(corner);
        }
        // Each edit takes the picked inside corners its own edges meet at.
        for edit in by_holder.values_mut() {
            let ends: Vec<Vec3> = edit.edges.iter().flat_map(|e| [e.a, e.b]).collect();
            edit.joints =
                world.joints.iter().copied().filter(|&j| ends.iter().any(|&p| (p - j).length() < 1e-6)).collect();
        }
        by_holder
            .into_iter()
            .filter_map(|(holder, edit)| Some((holder, edit.carried(&self.own_frame(holder, frames)?.inverse()))))
            .collect()
    }
}

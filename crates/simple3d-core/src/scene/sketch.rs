//! Sketch nodes (issue 74): a 2D sketch in the node's own XY plane and the solid made from the
//! regions picked out of it, extruded or revolved by the node's parameters.
//!
//! The sketch is kept whole, constraints and all, so it can be opened and changed again; the solid
//! is rebuilt from it on every evaluation. Its placement is the sketch plane: a sketch drawn on the
//! ground, a workplane or a face is a node turned to lie on it.

use super::*;
use crate::primitive::{ParamKind, ParamSpec, ParamValue, Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::{Mesh, Vec3};
use simple3d_sketch::feature::{self, Extrude};
use simple3d_sketch::{Shape, Sketch};
use std::collections::BTreeMap;
use std::sync::Arc;

/// What a sketch becomes.
pub const FEATURES: &[&str] = &["Extrude", "Revolve"];

/// Which way an extrusion runs from the sketch plane.
pub const EXTENTS: &[&str] = &["One side", "Two sides", "Symmetric"];

const fn spec(key: &'static str, label: &'static str, kind: ParamKind, default: ParamValue) -> ParamSpec {
    ParamSpec { key, label, kind, default, lock_group: 0, shown_when: None }
}

/// A sketch node's parameters; which of them apply depends on the feature and the extent, which the
/// panel decides ([`sketch_param_shown`]).
pub const SKETCH_PARAMS: [ParamSpec; 7] = [
    spec("feature", "Make", ParamKind::Choice { options: FEATURES }, ParamValue::Choice(0)),
    spec("extent", "Direction", ParamKind::Choice { options: EXTENTS }, ParamValue::Choice(0)),
    spec("distance", "Distance", ParamKind::Length { min: 0.01 }, ParamValue::Length(10.0)),
    spec("distance2", "Other side", ParamKind::Length { min: 0.0 }, ParamValue::Length(10.0)),
    spec("flip", "Reverse", ParamKind::Bool, ParamValue::Bool(false)),
    spec("draft", "Draft", ParamKind::Angle { min: -60.0, max: 60.0, wrap: false }, ParamValue::Angle(0.0)),
    spec("sweep", "Angle", ParamKind::Angle { min: 1.0, max: 360.0, wrap: false }, ParamValue::Angle(360.0)),
];

/// Whether parameter `key` means anything with `params` as they are.
pub fn sketch_param_shown(key: &str, params: &Params) -> bool {
    let revolve = params.int("feature") == 1;
    match key {
        "feature" | "flip" => true,
        "extent" | "distance" | "draft" => !revolve,
        "distance2" => !revolve && params.int("extent") == 1,
        "sweep" => revolve,
        _ => false,
    }
}

/// A sketch's parameters read from a file: each kept when it is the right kind and in range, else
/// the default.
pub fn sketch_params(stored: &Params) -> Params {
    SKETCH_PARAMS
        .iter()
        .map(|p| {
            let value = match (p.kind, stored.get(p.key)) {
                (ParamKind::Choice { options }, Some(&ParamValue::Choice(v))) if (v as usize) < options.len() => {
                    ParamValue::Choice(v)
                }
                (ParamKind::Length { min }, Some(&ParamValue::Length(v))) if v.is_finite() => {
                    ParamValue::Length(v.max(min))
                }
                (ParamKind::Angle { min, max, .. }, Some(&ParamValue::Angle(v))) if v.is_finite() => {
                    ParamValue::Angle(v.clamp(min, max))
                }
                (ParamKind::Bool, Some(&ParamValue::Bool(v))) => ParamValue::Bool(v),
                _ => p.default,
            };
            (p.key.to_string(), value)
        })
        .collect()
}

/// The solid `sketch` makes under `params`, with curves at `segments` to a full turn, in the sketch
/// node's own frame; or why there is none.
pub fn sketch_mesh(sketch: &Sketch, params: &Params, segments: u32) -> Result<Mesh, String> {
    if sketch.regions.is_empty() {
        return Err("No region of the sketch is picked to make a solid from".into());
    }
    let regions = simple3d_sketch::regions::find(sketch, segments);
    let picked = regions.resolve_all(&sketch.regions);
    if picked.is_empty() {
        return Err("The picked regions are no longer closed; open the sketch and pick them again".into());
    }
    let outlines = regions.outlines(&picked);
    let flip = params.flag("flip");
    if params.int("feature") == 1 {
        let axis = sketch.axis.and_then(|id| sketch.curve(id)).and_then(|curve| match curve.shape {
            Shape::Line { a, b } => Some((sketch.at(a), sketch.at(b))),
            _ => None,
        });
        let Some((a, b)) = axis else {
            return Err("Pick a line of the sketch as the revolve axis".into());
        };
        let axis = if flip { (b, a) } else { (a, b) };
        return feature::revolve(&outlines, axis, params.num("sweep"), segments);
    }
    let distance = params.num("distance");
    let (from, to) = match params.int("extent") {
        1 => (-params.num("distance2"), distance),
        2 => (-distance / 2.0, distance / 2.0),
        _ => (0.0, distance),
    };
    let (from, to) = if flip { (-to, -from) } else { (from, to) };
    feature::extrude(&outlines, Extrude { from, to, draft: params.num("draft") })
}

impl Scene {
    /// Whether any node is a sketch, which needs format 6.
    pub fn has_sketch(&self) -> bool {
        self.nodes.values().any(Node::is_sketch)
    }

    /// Add a sketch node whose plane is `plane` in the world. Drawn on a face of `owner`, it goes
    /// where push/pull's solids go -- beside the owner, or cut out of it -- else at the top of the
    /// tree, as a negative part when it cuts. `frames` is `Evaluated::node_frames`.
    pub fn add_sketch(
        &mut self,
        name: &str,
        sketch: Sketch,
        params: Params,
        plane: &Xform,
        owner: Option<(NodeId, Placing)>,
        frames: &BTreeMap<NodeId, Xform>,
    ) -> Option<NodeId> {
        let sketch = Arc::new(sketch);
        let lie = |parent: &Xform| {
            let local = parent.inverse().compose(plane);
            let (x, y) = (local.axis_vector(0).normalized(), local.axis_vector(1).normalized());
            (local.t, Xform::rotation_of_axes(x, y, x.cross(y)))
        };
        if let Some((owner, placing)) = owner.filter(|(owner, _)| self.nodes.contains_key(owner)) {
            let body = Body::Sketch { sketch: sketch.clone(), params: params.clone() };
            let placed = self.place_solid(owner, placing, false, name, frames, |parent| {
                let (position, rotation) = lie(parent);
                (body, position, rotation)
            });
            if placed.is_some() {
                return placed;
            }
        }
        let root = self.root;
        let top = self.nodes[&root].clone();
        let (position, rotation) =
            lie(&Xform::from_pos_rot_scale(top.position, top.rotation, Node::sane_scale(top.scale)));
        let index = top.children.len();
        let name = self.unique_name(name);
        let id = self.insert_fresh(name, Body::Sketch { sketch, params }, root, index);
        let node = self.nodes.get_mut(&id)?;
        node.position = position;
        node.rotation = rotation;
        node.negative = matches!(owner, Some((_, Placing::Cut)));
        Some(id)
    }

    /// Put a changed sketch and its parameters back on sketch node `id`.
    pub fn set_sketch(&mut self, id: NodeId, sketch: Sketch, params: Params) -> bool {
        match self.nodes.get_mut(&id).map(|node| &mut node.body) {
            Some(body @ Body::Sketch { .. }) => {
                *body = Body::Sketch { sketch: Arc::new(sketch), params };
                true
            }
            _ => false,
        }
    }
}

/// The world frame a sketch on node `id` is drawn in: its parent's frame and its placement.
pub fn sketch_frame(scene: &Scene, id: NodeId, frames: &BTreeMap<NodeId, Xform>) -> Option<Xform> {
    let node = scene.get(id)?;
    let parent = frames.get(&id).copied().unwrap_or_else(|| Xform::from_translation(Vec3::ZERO));
    Some(parent.compose(&Xform::from_pos_rot(node.position, node.rotation)))
}

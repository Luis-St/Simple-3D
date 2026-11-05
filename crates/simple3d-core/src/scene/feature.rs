//! Solids made from a flat profile (issue 74): the regions picked out of a sketch, or the outline
//! of a face of the model, extruded along the profile's normal by push/pull or revolved about a line
//! in its plane by the revolve tool. The profile lies in the node's own XY plane.
//!
//! A sketch profile holds a copy of its sketch, tied to the sketch node by [`Sketch::link`]: changing
//! the sketch puts the new copy on every feature made from it ([`Scene::set_sketch`]), so the solid
//! follows the drawing, while the feature still stands on its own in a file, on the clipboard or
//! with the sketch deleted.

use super::*;
use crate::primitive::{ParamKind, ParamSpec, ParamValue, Params, ParamsExt};
use crate::xform::Xform;
use serde::{Deserialize, Serialize};
use simple3d_geom::push_pull::Outline;
use simple3d_geom::{Mesh, Vec3};
use simple3d_sketch::feature::{self, Extrude};
use simple3d_sketch::{CurveId, RegionPick, Shape, Sketch};
use std::collections::BTreeMap;
use std::sync::Arc;

/// What a feature sweeps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// Regions of a sketch, found again by their curves whenever the sketch changes.
    Sketch { sketch: Sketch, regions: Vec<RegionPick> },
    /// A face's outline as it was captured.
    Face { outline: Outline },
}

/// The line a revolve turns about, in the profile's plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevolveAxis {
    /// A line of the profile's sketch, so the axis moves with the drawing.
    Line(CurveId),
    /// Two points of a line picked elsewhere: an edge of the model or an axis of the world.
    Fixed([f64; 2], [f64; 2]),
}

/// A profile and how it is swept: extruded when there is no axis, else revolved about it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub profile: Profile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis: Option<RevolveAxis>,
}

impl Feature {
    pub fn is_revolve(&self) -> bool {
        self.axis.is_some()
    }

    /// The sketch the profile was picked from, if it was.
    pub fn sketch(&self) -> Option<&Sketch> {
        match &self.profile {
            Profile::Sketch { sketch, .. } => Some(sketch),
            Profile::Face { .. } => None,
        }
    }
}

/// Which way an extrusion runs from the profile's plane.
pub const EXTENTS: &[&str] = &["One side", "Two sides", "Symmetric"];

const fn spec(key: &'static str, label: &'static str, kind: ParamKind, default: ParamValue) -> ParamSpec {
    ParamSpec { key, label, kind, default, lock_group: 0, shown_when: None }
}

/// An extruded feature's parameters; the second distance only means something on two sides
/// ([`feature_param_shown`]).
pub const EXTRUDE_PARAMS: [ParamSpec; 5] = [
    spec("extent", "Direction", ParamKind::Choice { options: EXTENTS }, ParamValue::Choice(0)),
    spec("distance", "Distance", ParamKind::Length { min: 0.01 }, ParamValue::Length(10.0)),
    spec("distance2", "Other side", ParamKind::Length { min: 0.0 }, ParamValue::Length(10.0)),
    spec("draft", "Draft", ParamKind::Angle { min: -60.0, max: 60.0, wrap: false }, ParamValue::Angle(0.0)),
    spec("flip", "Reverse", ParamKind::Bool, ParamValue::Bool(false)),
];

/// A revolved feature's parameters.
pub const REVOLVE_PARAMS: [ParamSpec; 2] = [
    spec("sweep", "Angle", ParamKind::Angle { min: 1.0, max: 360.0, wrap: false }, ParamValue::Angle(360.0)),
    spec("flip", "Reverse", ParamKind::Bool, ParamValue::Bool(false)),
];

/// The parameters a feature of this kind has.
pub fn feature_specs(revolve: bool) -> &'static [ParamSpec] {
    if revolve {
        &REVOLVE_PARAMS
    } else {
        &EXTRUDE_PARAMS
    }
}

/// Whether parameter `key` means anything with `params` as they are.
pub fn feature_param_shown(key: &str, params: &Params) -> bool {
    key != "distance2" || params.int("extent") == 1
}

/// A feature's parameters from stored ones: each kept when it is the right kind and in range, else
/// the default.
pub fn feature_params(revolve: bool, stored: &Params) -> Params {
    feature_specs(revolve)
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

/// The outlines a profile stands for, with curves at `segments` to a full turn.
pub fn profile_outlines(profile: &Profile, segments: u32) -> Result<Vec<Outline>, String> {
    match profile {
        Profile::Face { outline } => Ok(vec![outline.clone()]),
        Profile::Sketch { sketch, regions } => {
            let found = simple3d_sketch::regions::find(sketch, segments);
            let picked = found.resolve_all(regions);
            if picked.is_empty() {
                return Err("The sketch no longer closes the region this was made from".into());
            }
            Ok(found.outlines(&picked))
        }
    }
}

/// The solid `feature` makes under `params`, with curves at `segments` to a full turn, in the node's
/// own frame; or why there is none.
pub fn feature_mesh(feature: &Feature, params: &Params, segments: u32) -> Result<Mesh, String> {
    let outlines = profile_outlines(&feature.profile, segments)?;
    let flip = params.flag("flip");
    if let Some(axis) = &feature.axis {
        let line = match axis {
            RevolveAxis::Fixed(a, b) => Some((*a, *b)),
            RevolveAxis::Line(id) => feature.sketch().and_then(|sketch| match sketch.curve(*id)?.shape {
                Shape::Line { a, b } => Some((sketch.at(a), sketch.at(b))),
                _ => None,
            }),
        };
        let Some((a, b)) = line else {
            return Err("The line this revolves about is no longer in the sketch".into());
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
    /// Add a feature whose profile plane is `plane` in the world. Made on an object, it goes where
    /// push/pull's solids go -- beside the owner, or cut out of it -- else at the top of the tree.
    /// `frames` is `Evaluated::node_frames`.
    pub fn add_feature(
        &mut self,
        name: &str,
        feature: Feature,
        params: Params,
        plane: &Xform,
        owner: Option<(NodeId, Placing)>,
        frames: &BTreeMap<NodeId, Xform>,
    ) -> Option<NodeId> {
        let body = Body::Feature { feature: Arc::new(feature), params };
        if let Some((owner, placing)) = owner.filter(|(owner, _)| self.nodes.contains_key(owner)) {
            let placed = self.place_solid(owner, placing, false, name, frames, |parent| {
                let (position, rotation) = lie(parent, plane);
                (body.clone(), position, rotation)
            });
            if placed.is_some() {
                return placed;
            }
        }
        Some(self.add_at_top(name, body, plane))
    }

    /// Insert `body` last at the top of the tree, its own XY plane lying on `plane` in the world.
    pub(crate) fn add_at_top(&mut self, name: &str, body: Body, plane: &Xform) -> NodeId {
        let root = self.root;
        let top = &self.nodes[&root];
        let parent = Xform::from_pos_rot_scale(top.position, top.rotation, Node::sane_scale(top.scale));
        let (position, rotation) = lie(&parent, plane);
        let index = top.children.len();
        let name = self.unique_name(name);
        let id = self.insert_fresh(name, body, root, index);
        let node = self.nodes.get_mut(&id).expect("just inserted");
        node.position = position;
        node.rotation = rotation;
        id
    }

    /// Whether any node is a feature.
    pub fn has_feature(&self) -> bool {
        self.nodes.values().any(Node::is_feature)
    }
}

/// The position and rotation that lay a child of `parent` on `plane` in the world.
fn lie(parent: &Xform, plane: &Xform) -> (Vec3, Vec3) {
    let local = parent.inverse().compose(plane);
    let (x, y) = (local.axis_vector(0).normalized(), local.axis_vector(1).normalized());
    (local.t, Xform::rotation_of_axes(x, y, x.cross(y)))
}

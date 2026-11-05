//! Sketch nodes (issue 74): a 2D sketch in the node's own XY plane, kept whole, constraints and all,
//! so it can be opened and changed again. A sketch makes no solid itself; push/pull and the revolve
//! tool make [`Feature`]s from its regions, which follow it when it changes. Its placement is the
//! sketch plane: a sketch drawn on the ground, a workplane or a face is a node turned to lie on it.

use super::*;
use crate::primitive::{ParamValue, Params};
use crate::xform::Xform;
use simple3d_geom::Vec3;
use simple3d_sketch::Sketch;
use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A link no other sketch has: random per run, counted within it.
pub fn new_link() -> u64 {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(COUNT.fetch_add(1, Ordering::Relaxed));
    hasher.finish().max(1)
}

impl Scene {
    /// Whether any node is a sketch.
    pub fn has_sketch(&self) -> bool {
        self.nodes.values().any(Node::is_sketch)
    }

    /// Add a sketch node at the top of the tree whose plane is `plane` in the world.
    pub fn add_sketch(&mut self, name: &str, mut sketch: Sketch, plane: &Xform) -> NodeId {
        if sketch.link == 0 {
            sketch.link = new_link();
        }
        self.add_at_top(name, Body::Sketch { sketch: Arc::new(sketch) }, plane)
    }

    /// Put a changed sketch back on sketch node `id`, and its copy on every feature made from it.
    pub fn set_sketch(&mut self, id: NodeId, mut sketch: Sketch) -> bool {
        let Some(Body::Sketch { sketch: old }) = self.nodes.get(&id).map(|node| &node.body) else { return false };
        sketch.link = if old.link == 0 { new_link() } else { old.link };
        let link = sketch.link;
        for node in self.nodes.values_mut() {
            let Body::Feature { feature, .. } = &mut node.body else { continue };
            if feature.sketch().is_some_and(|copy| copy.link == link) {
                if let Profile::Sketch { sketch: copy, .. } = &mut Arc::make_mut(feature).profile {
                    *copy = sketch.clone();
                }
            }
        }
        if let Some(node) = self.nodes.get_mut(&id) {
            node.body = Body::Sketch { sketch: Arc::new(sketch) };
        }
        true
    }

    /// The features made from sketch `link`, in id order.
    pub fn features_of(&self, link: u64) -> Vec<NodeId> {
        let made = |node: &&Node| node.feature().and_then(|f| f.sketch()).is_some_and(|s| s.link == link);
        self.nodes.values().filter(made).map(|node| node.id).collect()
    }

    /// The sketch node with link `link`, if it is still in the document.
    pub fn sketch_node(&self, link: u64) -> Option<NodeId> {
        self.nodes.values().find(|node| node.sketch().is_some_and(|s| s.link == link)).map(|node| node.id)
    }

    /// Give every sketch under `tops` a link of its own, carried over to the features under `tops`
    /// made from it, so a pasted or duplicated sketch changes along with its own copied features and
    /// not with the originals. A copied feature whose sketch was not copied keeps following it.
    pub fn relink(&mut self, tops: &[NodeId]) {
        let ids: Vec<NodeId> = tops.iter().flat_map(|&top| self.subtree_ids(top)).collect();
        let fresh: BTreeMap<u64, u64> = ids
            .iter()
            .filter_map(|id| self.nodes.get(id)?.sketch().map(|s| s.link))
            .filter(|&link| link != 0)
            .map(|link| (link, new_link()))
            .collect();
        for id in ids {
            let Some(body) = self.nodes.get_mut(&id).map(|node| &mut node.body) else { continue };
            match body {
                Body::Sketch { sketch } => {
                    if let Some(&link) = fresh.get(&sketch.link) {
                        Arc::make_mut(sketch).link = link;
                    }
                }
                Body::Feature { feature, .. } => {
                    let Some(&link) = feature.sketch().and_then(|s| fresh.get(&s.link)) else { continue };
                    if let Profile::Sketch { sketch, .. } = &mut Arc::make_mut(feature).profile {
                        sketch.link = link;
                    }
                }
                _ => {}
            }
        }
    }

    /// `id` and everything under it, parents first.
    fn subtree_ids(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = vec![id];
        let mut k = 0;
        while k < out.len() {
            if let Some(node) = self.nodes.get(&out[k]) {
                out.extend(node.children.iter().copied());
            }
            k += 1;
        }
        out
    }
}

/// A sketch node of a file from before features were nodes of their own (format 6): the sketch
/// with its picks taken off, and the feature it made, if it picked anything. The feature takes
/// the old node's place in the tree, so the model is unchanged; the sketch goes beside it.
pub(crate) fn split_old_sketch(mut sketch: Sketch, params: &Params) -> (Sketch, Option<(Feature, Params)>) {
    sketch.link = new_link();
    let regions = std::mem::take(&mut sketch.regions);
    let axis = sketch.axis.take();
    if regions.is_empty() {
        return (sketch, None);
    }
    let revolve = matches!(params.get("feature"), Some(ParamValue::Choice(1)));
    // An old revolve with no axis still reads as one, and says what is missing.
    let axis = revolve.then(|| axis.map_or(RevolveAxis::Fixed([0.0; 2], [0.0; 2]), RevolveAxis::Line));
    let feature = Feature { profile: Profile::Sketch { sketch: sketch.clone(), regions }, axis };
    let params = feature_params(revolve, params);
    (sketch, Some((feature, params)))
}

/// Whether a stored sketch node is in the form before format 7, which kept its feature's parameters.
pub(crate) fn is_old_sketch(params: &Params) -> bool {
    params.contains_key("feature")
}

/// The world frame a sketch on node `id` is drawn in: its parent's frame and its placement.
pub fn sketch_frame(scene: &Scene, id: NodeId, frames: &BTreeMap<NodeId, Xform>) -> Option<Xform> {
    let node = scene.get(id)?;
    let parent = frames.get(&id).copied().unwrap_or_else(|| Xform::from_translation(Vec3::ZERO));
    Some(parent.compose(&Xform::from_pos_rot(node.position, node.rotation)))
}

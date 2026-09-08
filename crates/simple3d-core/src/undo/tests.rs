mod bounds;
mod coalescing;
mod steps;

use super::*;
use crate::scene::NodeId;
use crate::scene::Scene;

fn shape(scene: &mut Scene) -> NodeId {
    let root = scene.root();
    let index = scene.node(root).children.len();
    scene.add_primitive("box", root, index).unwrap()
}

/// A structural fingerprint of the tree, so "identical" can be asserted
/// without relying on node ids being reused in the same order.
fn fingerprint(scene: &Scene) -> String {
    let mut out = String::new();
    for id in scene.depth_first() {
        let node = scene.node(id);
        out.push_str(&format!(
            "{}|{}|{:?}|{:?}|{:?}|{:?}|{}|{}|{:?};",
            scene.depth(id),
            node.name,
            node.position,
            node.rotation,
            node.scale,
            node.anchor,
            node.visible,
            node.segments.unwrap_or(0),
            node.params().cloned().unwrap_or_default(),
        ));
    }
    out
}

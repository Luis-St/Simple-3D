//! Which node the pointer is over.

use super::*;
use simple3d_core::eval::Evaluated;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_geom::Vec3;

/// The visible node the ray hits first; hidden nodes and their subtrees are clicked through.
pub fn pick(scene: &Scene, evaluated: &Evaluated, origin: Vec3, dir: Vec3) -> Option<NodeId> {
    // A pattern and its repeated child can hit at the same distance. The first found used to win, making
    // the answer depend on node ids; the enclosing node wins the tie now.
    const SAME_HIT: f64 = 1e-9;
    let mut best: Option<(f64, NodeId)> = None;
    for (&id, mesh) in &evaluated.node_meshes {
        if !scene.contains(id) || !scene.is_shown(id) {
            continue;
        }
        if let Some(t) = ray_mesh(mesh, origin, dir) {
            let wins = match best {
                None => true,
                Some((bt, best_id)) => {
                    t < bt - SAME_HIT || ((t - bt).abs() <= SAME_HIT && scene.is_ancestor_of(id, best_id))
                }
            };
            if wins {
                best = Some((t, id));
            }
        }
    }
    // What push/pull added to a boolean (issue 73) is in no operand's mesh, so the boolean's result
    // is tried too. It only wins where it is in front of every operand: elsewhere its surface is one.
    for &id in evaluated.group_meshes.keys() {
        let Some(node) = scene.get(id) else { continue };
        if !node.edits.iter().any(|edit| edit.adds()) || !scene.is_shown(id) {
            continue;
        }
        let Some(mesh) = evaluated.result_mesh(id) else { continue };
        if let Some(t) = super::ray::ray_mesh_linear(&mesh, origin, dir) {
            if best.map_or(true, |(bt, _)| t < bt - 1e-6) {
                best = Some((t, id));
            }
        }
    }
    best.map(|(_, id)| id)
}

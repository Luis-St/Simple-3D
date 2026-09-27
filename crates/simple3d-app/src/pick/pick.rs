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
    best.map(|(_, id)| id)
}

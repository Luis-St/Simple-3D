//! Which node the pointer is over.

use super::*;
use simple3d_core::eval::Evaluated;
use simple3d_core::scene::{NodeId, Scene};
use simple3d_geom::Vec3;

/// The visible node the ray hits first. Hidden nodes are skipped, and so is
/// anything under a hidden group: a hidden node is not part of the model, so a
/// click passes through it to whatever is actually there.
pub fn pick(scene: &Scene, evaluated: &Evaluated, origin: Vec3, dir: Vec3) -> Option<NodeId> {
    // Two meshes can answer at exactly the same distance: a pattern puts its
    // whole repeated result under its own id while the child it repeats keeps
    // its mesh too, so over the original copy the surfaces are the same
    // triangles. Whichever the map happened to reach first used to win, which
    // made the answer depend on node ids -- a pattern made by wrapping a shape
    // selected the child over the original copy and the pattern over every
    // other, while one filled after the fact answered "pattern" everywhere. The
    // enclosing node wins the tie, so a click anywhere on a pattern's output
    // means the same thing.
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

mod arrange;
mod collection;
mod colour;
mod edit;
mod naming;
mod portable;
mod split;

use super::*;
use simple3d_geom::Vec3;

fn box_at(scene: &mut Scene, parent: NodeId, x: f64) -> NodeId {
    let index = scene.node(parent).children.len();
    let id = scene.add_primitive("box", parent, index).unwrap();
    scene.get_mut(id).unwrap().position = Vec3::new(x, 0.0, 0.0);
    id
}

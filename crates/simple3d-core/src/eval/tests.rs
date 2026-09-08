mod bake;
mod boolean;
mod cache;
mod cancel;
mod colour;
mod groups;
mod pattern;
mod scale;

use super::*;
use crate::primitive::ParamValue;
use crate::scene::NodeId;
use crate::scene::Scene;
use simple3d_geom::{Mesh, Vec3};

fn plate(scene: &mut Scene, parent: NodeId) -> NodeId {
    let index = scene.node(parent).children.len();
    scene.add_primitive("plate", parent, index).unwrap()
}

fn cylinder(scene: &mut Scene, parent: NodeId, diameter: f64, height: f64) -> NodeId {
    let index = scene.node(parent).children.len();
    let id = scene.add_primitive("cylinder", parent, index).unwrap();
    let params = scene.get_mut(id).unwrap().params_mut().unwrap();
    params.insert("diameter_x".into(), ParamValue::Length(diameter));
    params.insert("diameter_y".into(), ParamValue::Length(diameter));
    params.insert("height".into(), ParamValue::Length(height));
    id
}

fn size(mesh: &Mesh) -> Vec3 {
    let (lo, hi) = mesh.bounds().unwrap();
    hi - lo
}

/// Every colour a mesh's faces are painted, with how many faces each has.
fn painted(mesh: &Mesh) -> std::collections::BTreeMap<Option<[u8; 3]>, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for i in 0..mesh.indices.len() {
        *counts.entry(crate::scene::Colour::from_tag(mesh.tag(i)).map(|c| c.0)).or_insert(0) += 1;
    }
    counts
}

mod fields;
mod migration;
mod refusal;
mod round_trip;

use super::*;
use crate::primitive::ParamValue;
use crate::scene::Scene;
use crate::scene::{Anchor, GroupOp};
use crate::unit::Unit;
use simple3d_geom::Vec3;

fn sample() -> Scene {
    let mut scene = Scene::new();
    scene.settings.unit = Unit::Centimetre;
    scene.settings.default_segments = 48;
    scene.settings.notes = "A bracket".into();
    scene.settings.grid_spacing = 5.0;
    scene.camera.distance = 321.5;
    scene.camera.yaw = -12.5;
    scene.camera.pitch = 41.25;
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    scene.get_mut(group).unwrap().name = "Drilled plate".into();
    let plate = scene.add_primitive("plate", group, 0).unwrap();
    scene.get_mut(plate).unwrap().anchor = Anchor::Base;
    let hole = scene.add_primitive("cylinder", group, 1).unwrap();
    scene.get_mut(hole).unwrap().position = Vec3::new(-8.0, 0.0, 0.0);
    scene.get_mut(hole).unwrap().rotation = Vec3::new(0.0, 0.0, 30.0);
    scene.get_mut(hole).unwrap().segments = Some(64);
    scene.get_mut(hole).unwrap().visible = false;
    scene.get_mut(plate).unwrap().scale = Vec3::new(1.5, 1.0, 0.25);
    scene.get_mut(hole).unwrap().params_mut().unwrap().insert("diameter_x".into(), ParamValue::Length(6.0));
    scene
}

fn fingerprint(scene: &Scene) -> String {
    let mut out = format!("{:?}{:?}", scene.settings, scene.camera);
    for id in scene.depth_first() {
        let node = scene.node(id);
        out.push_str(&format!(
            "{}|{}|{:?}|{:?}|{:?}|{:?}|{}|{:?}|{:?};",
            scene.depth(id),
            node.name,
            node.position,
            node.rotation,
            node.scale,
            node.anchor,
            node.visible,
            node.segments,
            node.params(),
        ));
    }
    out
}

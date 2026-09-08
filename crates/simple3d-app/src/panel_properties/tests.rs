mod fields;
mod panels;

use super::edit::*;
use super::point::*;
use super::primitive::*;
use super::scrub::*;
use super::selection::*;
use crate::app::App;
use simple3d_core::primitive;
use simple3d_core::primitive::ParamValue;
use simple3d_core::primitive::ParamsExt;
use simple3d_core::scene::NodeId;
use simple3d_core::scene::Scene;

/// An `App` on a headless context, pointed at a throwaway config directory
/// so a test cannot read or write the developer's own.
fn headless_app() -> App {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-props-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    App::with_config_dir(&egui::Context::default(), None, dir)
}

/// Two plates of different widths, selected together.
fn two_plates(app: &mut App) -> (NodeId, NodeId) {
    let root = app.scene.root();
    let a = app.scene.add_primitive("plate", root, 0).unwrap();
    let b = app.scene.add_primitive("plate", root, 1).unwrap();
    set_param(app, a, "width", ParamValue::Length(40.0));
    set_param(app, b, "width", ParamValue::Length(60.0));
    app.selection = vec![a, b];
    (a, b)
}

fn width_of(app: &App, id: NodeId) -> f64 {
    app.scene.node(id).params().unwrap().num("width")
}

fn width_spec() -> &'static primitive::ParamSpec {
    primitive::lookup("plate").unwrap().params.iter().find(|p| p.key == "width").unwrap()
}

/// The same rule as `is_locked`, against a bare scene so it can be tested
/// without an `App`.
fn locked_in(scene: &Scene, id: NodeId, group: u8) -> bool {
    let spec = scene.node(id).spec().unwrap();
    let params = scene.node(id).params().unwrap();
    let keys: Vec<&str> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key).collect();
    let (first, rest) = keys.split_first().unwrap();
    let reference = params.num(first);
    rest.iter().all(|k| (params.num(k) - reference).abs() < 1e-9)
}

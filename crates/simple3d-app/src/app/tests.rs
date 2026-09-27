mod bounds;
mod camera;
mod components;
mod csg;
mod document;
mod drag;
mod keymap;
mod library;
mod outliner;
mod panels;
mod pattern_kind;
mod pattern_stages;
mod pattern_variations;
mod placement;
mod settings;
mod steps;
pub(crate) use drag::*;
mod export;
mod export_bodies;
mod file_dialog;
mod import;
mod measure;
mod measure_axis;
mod measure_hidden;
mod nudge;
mod paint;
mod pattern_grips;
mod pattern_tool;
mod pattern_tool_layout;
mod tabs;
mod windows;
pub(crate) use pattern_grips::*;
mod mesh;
mod pattern_edit;
mod reassemble;
mod simplify;
mod snap;
mod snap_resize;
mod split;
pub(crate) use split::*;
mod pieces;
mod preview;

use super::*;
use simple3d_core::eval::{Cancel, Evaluator};
use std::path::PathBuf;

/// A temporary config directory with default settings and keymap, so tests do not depend on the
/// developer's own config.
pub(crate) fn temp_config_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-app-test-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An app with one plate in it, since the application opens on an empty document.
pub(crate) fn headless_app() -> App {
    let mut app = app_in(temp_config_dir("headless"));
    let root = app.scene.root();
    let id = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    app.selection = vec![id];
    app.history.clear();
    app.saved_revision = app.history.revision();
    app.reevaluate_for_test();
    app
}

/// A fresh app holding two boxes, the second moved to `at` and the first selected.
pub(crate) fn two_boxes(
    tag: &str,
    at: simple3d_geom::Vec3,
) -> (App, simple3d_core::scene::NodeId, simple3d_core::scene::NodeId) {
    let mut app = app_in(temp_config_dir(tag));
    let root = app.scene.root();
    let a = app.scene.add_primitive("box", root, 0).unwrap();
    let b = app.scene.add_primitive("box", root, 1).unwrap();
    app.scene.get_mut(b).unwrap().position = at;
    app.select_only(a);
    (app, a, b)
}

fn app_in(config_dir: PathBuf) -> App {
    let ctx = egui::Context::default();
    let mut app = App::with_config_dir(&ctx, None, config_dir);
    // The gizmo needs a viewport for screen-facing axes, and an evaluation for the node's place.
    app.viewport_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
    app.reevaluate_for_test();
    app
}

/// Draw one whole frame headlessly, so panics and bad layouts fail here.
pub(crate) fn draw_one_frame(app: &mut App) {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    // A real window size, since the default screen rect is effectively infinite.
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 880.0))),
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| app.ui(ctx));
}

/// Draw one frame with given modifiers and key events, so modifier-only bindings can be typed
/// (issue 76).
fn draw_frame_with(app: &mut App, modifiers: egui::Modifiers, events: Vec<egui::Event>) {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 880.0))),
        modifiers,
        events,
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| app.ui(ctx));
}

impl App {
    pub(crate) fn reevaluate_for_test(&mut self) {
        self.evaluated = Evaluator::new().evaluate(&self.scene, &Cancel::new());
        // Bump the generation as the worker does, since change watchers key on it.
        self.evaluation_generation += 1;
    }
}

/// An app with the tool open on a pattern of the starting shape, started from a linear run.
fn with_rule() -> (App, simple3d_core::scene::NodeId) {
    let mut app = headless_app();
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool opened on a pattern");
    app.start_rule_from(pattern, 0);
    app.reevaluate_for_test();
    (app, pattern)
}

fn params(app: &App, id: simple3d_core::scene::NodeId) -> simple3d_core::primitive::Params {
    app.scene.node(id).params().cloned().expect("a pattern has parameters")
}

mod camera;
mod document;
mod drag;
mod keymap;
mod library;
mod outliner;
mod panels;
mod pattern_kind;
mod placement;
mod settings;
mod steps;
pub(crate) use drag::*;
mod export;
mod export_bodies;
mod file_dialog;
mod measure;
mod measure_axis;
mod measure_hidden;
mod nudge;
mod paint;
mod pattern_grips;
mod pattern_preview;
mod pattern_tool;
mod pattern_tool_layout;
mod tabs;
pub(crate) use pattern_grips::*;
mod mesh;
mod pattern_edit;
mod snap;
mod snap_resize;
mod split;
pub(crate) use split::*;
mod pieces;
mod preview;

use super::*;
use simple3d_core::eval::{Cancel, Evaluator};
use std::path::PathBuf;

/// An `App` on a headless `egui::Context`, which needs no window and no
/// graphics -- so the command dispatch itself can be driven from a test.
///
/// `App::new` reads the *user's real* settings and keymap, so both are put
/// back to their defaults here; a test must not change its answer because of
/// what is in the developer's config directory.
fn temp_config_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-app-test-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An app with one plate in it. The application itself now opens on an
/// empty document, so the tests below -- which are about what happens *to*
/// a shape -- put the shape there themselves.
fn headless_app() -> App {
    let mut app = app_in(temp_config_dir("headless"));
    let root = app.scene.root();
    let id = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    app.selection = vec![id];
    app.history.clear();
    app.saved_revision = app.history.revision();
    app.reevaluate_for_test();
    app
}

fn app_in(config_dir: PathBuf) -> App {
    let ctx = egui::Context::default();
    let mut app = App::with_config_dir(&ctx, None, config_dir);
    // The gizmo needs a viewport to work out which axes face the screen, and
    // an evaluation to know where the node is.
    app.viewport_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
    app.reevaluate_for_test();
    app
}

/// Draw one entire frame of the interface into a headless context. A panel
/// that panics, or a layout that divides by a width it does not have, fails
/// here rather than in front of someone.
fn draw_one_frame(app: &mut App) {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    // A real window size: the default raw input has an effectively infinite
    // screen rect, and the viewport would ask for a texture larger than any
    // GPU allows.
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 880.0))),
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| app.ui(ctx));
}

/// Draw one frame with a given modifier state and key events, so a binding
/// that is nothing but a held modifier can be typed at the application the
/// way a hand types it (issue 76).
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
    pub(super) fn reevaluate_for_test(&mut self) {
        self.evaluated = Evaluator::new().evaluate(&self.scene, &Cancel::new());
        // The generation moves on with the result, exactly as it does when
        // the worker hands one back: everything that watches for "the model
        // changed" watches this, so a helper that left it alone would be a
        // helper nothing noticed.
        self.evaluation_generation += 1;
    }
}

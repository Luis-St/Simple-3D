//! Pointer gestures, performed through a headless harness.
//!
//! Drives `App::handle_shortcuts` and `App::ui` as the running window does and replays real
//! pointer events with `egui_kittest`, testing the wiring between widgets (which one claims a drag,
//! whether the view cube also orbits, click versus drag on a header) rather than the arithmetic,
//! which is tested elsewhere. Positions come from named widget ids, so layout changes move the
//! tests with them.

mod camera;
mod docks;
mod fields;
mod fields_tab;
mod handles;
mod scrub;
mod section;
pub(crate) use camera::*;
mod arrange_tool;
mod cursor;
mod keys;
mod measure;
mod noise_popup;
mod outliner_drag;
mod outliner_menu;
mod outliner_select;
mod palette_drag;
mod pattern_tool;
mod push_pull;
mod round_tool;
mod round_tool_joins;
mod snap;
mod split_tool;
mod tab_drag;
mod view_cube;

use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::eval::{Cancel, Evaluator};

/// An `App` on its own config directory, in a harness drawing one real frame per step.
fn harness(name: &str) -> Harness<'static, App> {
    harness_configured(name, |_| {})
}

/// The same, with settings changed before the first frame (egui remembers some, like dock
/// widths, from the first frame).
fn harness_configured(name: &str, setup: impl FnOnce(&mut App)) -> Harness<'static, App> {
    // A quarter second per frame, which everything but the click tests wants.
    harness_stepping(name, 1.0 / 4.0, setup)
}

/// The same, with an explicit clock step, since double clicks need two clicks close in time.
fn harness_stepping(name: &str, step_dt: f32, setup: impl FnOnce(&mut App)) -> Harness<'static, App> {
    let dir = crate::app::tests::temp_config_dir(name);

    let mut app = App::with_config_dir(&egui::Context::default(), None, dir);
    // The app opens empty, so add the shape the gestures act on.
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    app.select_only(plate);
    // Evaluate synchronously so picking, the manipulator and the cursor have geometry.
    app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    app.frame_all();
    app.history.clear();
    setup(&mut app);

    let mut themed = false;
    let mut harness = Harness::builder().with_size(egui::vec2(1400.0, 880.0)).with_step_dt(step_dt).build_state(
        move |ctx, app: &mut App| {
            if !themed {
                // The harness makes its own context, so it is themed here, as `App::new` would.
                crate::theme::apply(ctx);
                themed = true;
            }
            // Keyboard before panels, as in the running window.
            app.handle_shortcuts(ctx);
            app.ui(ctx);
        },
        app,
    );
    // Two frames: one to lay out, one to interact with that layout.
    harness.step();
    harness.step();
    harness
}

/// Where a named widget was drawn in the last frame.
#[track_caller]
fn rect_of(harness: &Harness<'_, App>, id: egui::Id) -> egui::Rect {
    harness.ctx.read_response(id).unwrap_or_else(|| panic!("{id:?} was not drawn")).rect
}

fn event(harness: &mut Harness<'_, App>, event: egui::Event) {
    harness.input_mut().events.push(event);
    harness.step();
}

/// Hold or release modifier keys for everything that follows.
fn modifiers(harness: &mut Harness<'_, App>, modifiers: egui::Modifiers) {
    harness.input_mut().modifiers = modifiers;
}

fn move_to(harness: &mut Harness<'_, App>, pos: egui::Pos2) {
    event(harness, egui::Event::PointerMoved(pos));
}

fn button(harness: &mut Harness<'_, App>, pos: egui::Pos2, button: egui::PointerButton, pressed: bool) {
    let modifiers = harness.input().modifiers;
    event(harness, egui::Event::PointerButton { pos, button, pressed, modifiers });
}

fn press(harness: &mut Harness<'_, App>, pos: egui::Pos2) {
    move_to(harness, pos);
    button(harness, pos, egui::PointerButton::Primary, true);
}

fn release(harness: &mut Harness<'_, App>, pos: egui::Pos2) {
    button(harness, pos, egui::PointerButton::Primary, false);
    harness.step();
}

/// Press at `from`, move to `to` over several frames, release; egui only sees a drag after movement.
fn drag(harness: &mut Harness<'_, App>, from: egui::Pos2, to: egui::Pos2, frames: usize) {
    press(harness, from);
    for frame in 1..=frames {
        let t = frame as f32 / frames as f32;
        move_to(harness, from + (to - from) * t);
    }
    release(harness, to);
}

// -- a value field: Enter takes it, Escape leaves it ---------------------------

fn text(harness: &mut Harness<'_, App>, what: &str) {
    event(harness, egui::Event::Text(what.to_string()));
}

/// Click into a field, select its contents and type over them.
fn replace_field(harness: &mut Harness<'_, App>, at: egui::Pos2, what: &str) {
    press(harness, at);
    release(harness, at);
    modifiers(harness, egui::Modifiers::COMMAND);
    key(harness, egui::Key::A);
    modifiers(harness, egui::Modifiers::NONE);
    text(harness, what);
}

/// Press or release one key and leave it so, for key combinations.
fn hold_key(harness: &mut Harness<'_, App>, key: egui::Key, pressed: bool) {
    let modifiers = harness.input().modifiers;
    event(harness, egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers });
    harness.step();
}

fn key(harness: &mut Harness<'_, App>, key: egui::Key) {
    let modifiers = harness.input().modifiers;
    event(harness, egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    event(harness, egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
}

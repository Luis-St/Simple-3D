//! The gestures a pointer has to perform, performed.
//!
//! Everything in here drives `App::handle_shortcuts` and then `App::ui` -- the
//! same keyboard pass and the same panels, in the same order, that the running
//! window draws -- against a headless context, and replays real pointer events
//! over it with `egui_kittest`. Nothing calls the arithmetic
//! underneath a gesture directly; that is covered elsewhere (`dock::drop_index`,
//! `ui::scrub_delta`, `view::cube_face_at`, `panel_properties::scrub_param`).
//! What is covered *here* is the wiring in between, which those tests cannot
//! see: which widget claims a drag, whether a press on the view cube also orbits
//! the camera behind it, and whether a click on a panel header is told apart
//! from a drag of it.
//!
//! Two of the last three passes shipped a fault that every test passed through
//! and only driving the window revealed. This is that driving, in a test.
//!
//! **Positions come from the widget, not from a guess.** Each grip has an id it
//! is named by rather than one taken from where it sits (`dock::header_id`,
//! `panel_properties::grip_id`, `panel_viewport::cube_id`), so a test asks the
//! context where that widget was drawn and puts the pointer there. A layout
//! change moves the test with it.

mod camera;
mod docks;
mod fields;
mod handles;
mod scrub;
mod section;
pub(crate) use camera::*;
mod cursor;
mod keys;
mod measure;
mod outliner_drag;
mod outliner_menu;
mod outliner_select;
mod palette_drag;
mod pattern_tool;
mod snap;
mod split_tool;
mod view_cube;

use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::eval::{Cancel, Evaluator};

/// An `App` on its own config directory, wired into a harness that draws one
/// real frame per step.
fn harness(name: &str) -> Harness<'static, App> {
    harness_configured(name, |_| {})
}

/// The same, with a chance to change the application's settings before the
/// first frame is drawn -- a dock width, say, which egui remembers from the
/// frame the panel first appeared in and not from the setting afterwards.
fn harness_configured(name: &str, setup: impl FnOnce(&mut App)) -> Harness<'static, App> {
    // The harness's default step: a quarter of a second of egui's clock per
    // frame, which is what everything below but the click tests wants.
    harness_stepping(name, 1.0 / 4.0, setup)
}

/// The same, with the step the harness's clock advances by each frame said out
/// loud -- because egui tells a double click from two clicks by the delay
/// between them, and at the default quarter of a second no two clicks a test
/// can make are ever close enough to be one.
fn harness_stepping(name: &str, step_dt: f32, setup: impl FnOnce(&mut App)) -> Harness<'static, App> {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-gesture-test-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut app = App::with_config_dir(&egui::Context::default(), None, dir);
    // The application opens on an empty document, and every gesture below needs
    // something to perform itself on, so the shape the starter scene used to
    // hold is put there here.
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    app.select_only(plate);
    // The worker is asynchronous, and a test must not wait on it: evaluate the
    // scene here so picking, the manipulator and the 3D cursor have geometry to
    // work against.
    app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    app.frame_all();
    app.history.clear();
    setup(&mut app);

    let mut themed = false;
    let mut harness = Harness::builder().with_size(egui::vec2(1400.0, 880.0)).with_step_dt(step_dt).build_state(
        move |ctx, app: &mut App| {
            if !themed {
                // `App::new` themes the context it is handed; the harness makes
                // its own, so it is themed here instead. Sizes come from the
                // theme, so a frame drawn without it is not the real frame.
                crate::theme::apply(ctx);
                themed = true;
            }
            // The same order the running window draws in: the frame's keyboard
            // is read before the panels are laid out, so a test can hold a key
            // as well as move a pointer.
            app.handle_shortcuts(ctx);
            app.ui(ctx);
        },
        app,
    );
    // Two frames: the first lays the panels out, the second interacts against
    // that layout, which is how egui works and therefore what a test must do.
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

/// Hold (or let go of) modifier keys for everything that follows. `RawInput`
/// carries them across frames, which is what holding a key means.
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

/// Press at `from`, cross to `to` over several frames, release there. Several
/// frames because a drag is not a jump: egui decides a press is a drag only once
/// the pointer has moved, and the application re-lays its panels out in between.
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

/// Click into a field, select what is in it, and type over it -- which is what
/// entering a value actually is.
fn replace_field(harness: &mut Harness<'_, App>, at: egui::Pos2, what: &str) {
    press(harness, at);
    release(harness, at);
    modifiers(harness, egui::Modifiers::COMMAND);
    key(harness, egui::Key::A);
    modifiers(harness, egui::Modifiers::NONE);
    text(harness, what);
}

/// Press or release one key and leave it in that state, which is what a
/// combination of several keys needs: `key` sends a press and a release
/// together, so nothing is ever down at the same time as anything else.
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

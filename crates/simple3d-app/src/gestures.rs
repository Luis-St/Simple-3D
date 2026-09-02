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

use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::config::{Panel, Side};
use simple3d_core::eval::{Cancel, Evaluator};
use simple3d_core::primitive::ParamsExt;
use simple3d_geom::Vec3;

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

// -- the dock header: click rolls up, drag moves ------------------------------

#[test]
fn dragging_a_panel_header_into_the_other_dock_moves_the_panel_there() {
    let mut harness = harness("dock-drag");
    assert_eq!(harness.state().settings.layout.side_of(Panel::Primitives), Side::Left);

    let header = rect_of(&harness, crate::dock::header_id(Panel::Primitives));
    let properties = rect_of(&harness, crate::dock::header_id(Panel::Properties));
    // Above the Properties header, which is the top of the right dock: the drop
    // index has to come out as nought.
    let target = egui::pos2(properties.center().x, properties.top() + 2.0);

    press(&mut harness, header.center());
    move_to(&mut harness, header.center() + egui::vec2(0.0, 24.0));
    assert_eq!(
        harness.state().dock_drag.panel,
        Some(Panel::Primitives),
        "the header did not claim the drag -- something under it did"
    );
    move_to(&mut harness, target);
    assert_eq!(
        harness.state().dock_drag.target,
        Some((Side::Right, 0)),
        "the drag was over the right dock's first slot and did not say so"
    );
    release(&mut harness, target);

    let layout = &harness.state().settings.layout;
    assert_eq!(layout.side_of(Panel::Primitives), Side::Right, "the panel did not move dock");
    assert_eq!(layout.right, vec![Panel::Primitives, Panel::Properties], "it landed in the wrong place");
    assert_eq!(layout.left, vec![Panel::Outliner]);
    assert!(harness.state().dock_drag.panel.is_none(), "the drag was left running after the button came up");
    assert!(!layout.is_collapsed(Panel::Primitives), "a drag also rolled the panel up");

    // And the panel is really in the other dock now: its header is drawn on the
    // right. The layout changed while the frame was being drawn, so the frame
    // that shows it is the next one.
    harness.step();
    let moved = rect_of(&harness, crate::dock::header_id(Panel::Primitives));
    assert!(moved.center().x > 1400.0 / 2.0, "the panel is in the layout but is not drawn on the right");
}

#[test]
fn clicking_a_panel_header_rolls_it_up_without_moving_it() {
    // The same widget carries both gestures, so the one thing that can go wrong
    // is that it cannot tell them apart.
    let mut harness = harness("dock-click");
    let header = rect_of(&harness, crate::dock::header_id(Panel::Outliner));
    let before = harness.state().settings.layout.clone();

    press(&mut harness, header.center());
    release(&mut harness, header.center());

    let layout = &harness.state().settings.layout;
    assert!(layout.is_collapsed(Panel::Outliner), "a click on the header did not roll the panel up");
    assert_eq!(layout.left, before.left, "a click moved the panel as well");
    assert_eq!(layout.right, before.right);

    // Clicking it again unrolls it: the header is the whole of the interface.
    let header = rect_of(&harness, crate::dock::header_id(Panel::Outliner));
    press(&mut harness, header.center());
    release(&mut harness, header.center());
    assert!(!harness.state().settings.layout.is_collapsed(Panel::Outliner), "the second click did not unroll it");
}

// -- the scrub, which is on the field itself ----------------------------------

#[test]
fn dragging_a_value_field_scrubs_it_in_one_undo_step() {
    let mut harness = harness("scrub");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    let width = |harness: &Harness<'_, App>| harness.state().scene.node(plate).params().unwrap().num("width");
    assert_eq!(width(&harness), 40.0);
    let steps = harness.state().history.undo_len();

    // Sixty pixels to the right at six pixels a millimetre is ten millimetres.
    let grip = rect_of(&harness, crate::panel_properties::grip_id("Width (X)"));
    drag(&mut harness, grip.center(), grip.center() + egui::vec2(60.0, 0.0), 6);

    assert!((width(&harness) - 50.0).abs() < 1e-9, "the scrub gave {} rather than 50 mm", width(&harness));
    assert_eq!(
        harness.state().history.undo_len(),
        steps + 1,
        "a scrub across six frames left more than one thing to undo"
    );
    assert!(harness.state().scrub.id.is_none(), "the scrub was left running after the button came up");

    harness.state_mut().run(simple3d_core::keymap::Command::Undo);
    assert_eq!(width(&harness), 40.0, "one undo did not take the whole scrub back");
}

#[test]
fn a_scrub_that_runs_off_its_field_keeps_scrubbing_that_field_and_no_other() {
    // The pointer leaves the field almost immediately -- six pixels a
    // millimetre means any real edit crosses it -- and it passes over the other
    // two dimension fields on its way. Neither may take the gesture over.
    let mut harness = harness("scrub-away");
    let plate = harness.state().primary().unwrap();
    let params = |harness: &Harness<'_, App>| {
        let p = harness.state().scene.node(plate).params().unwrap().clone();
        (p.num("width"), p.num("depth"), p.num("thickness"))
    };
    assert_eq!(params(&harness), (40.0, 20.0, 4.0));

    let grip = rect_of(&harness, crate::panel_properties::grip_id("Width (X)"));
    let depth = rect_of(&harness, crate::panel_properties::grip_id("Depth (Y)"));
    press(&mut harness, grip.center());
    // Down onto the depth label, and 30 px across it.
    move_to(&mut harness, egui::pos2(grip.center().x, depth.center().y));
    move_to(&mut harness, egui::pos2(grip.center().x + 30.0, depth.center().y));
    release(&mut harness, egui::pos2(grip.center().x + 30.0, depth.center().y));

    let (w, d, t) = params(&harness);
    assert!((w - 45.0).abs() < 1e-9, "the field the drag began on scrubbed to {w} rather than 45 mm");
    assert_eq!((d, t), (20.0, 4.0), "a field the pointer merely crossed was scrubbed too");
}

#[test]
fn every_scrubbable_value_is_dragged_from_its_own_field() {
    // Issue 27: the drag used to be on whatever sat beside the field -- the
    // label for a dimension, the four-pixel axis chip for a position -- so the
    // same gesture had a different grip depending on the row. Every one of them
    // is now the field, and this drags all three kinds to prove it.
    let mut harness = harness("scrub-everywhere");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    let node = |harness: &Harness<'_, App>| harness.state().scene.node(plate).clone();
    let before = node(&harness);

    for (name, moved) in [("Position (mm):0", 10.0), ("Rotation (deg):2", 150.0), ("Scale (x):1", 0.5)] {
        let field = rect_of(&harness, crate::panel_properties::grip_id(name));
        drag(&mut harness, field.center(), field.center() + egui::vec2(60.0, 0.0), 6);
        let after = node(&harness);
        let change = match name {
            "Position (mm):0" => after.position.x - before.position.x,
            "Rotation (deg):2" => after.rotation.z - before.rotation.z,
            _ => after.scale.y - before.scale.y,
        };
        assert!((change - moved).abs() < 1e-9, "dragging {name} moved it by {change} rather than {moved}");
    }
}

// -- picking and grabbing in the viewport -------------------------------------

#[test]
fn clicking_a_shape_selects_it_when_nothing_is_selected_yet() {
    // Picking used to live inside the manipulator, which returns early when
    // there is no primary node -- so with an empty selection the first click
    // into the viewport did nothing at all, and the outliner was the only way
    // in. Clicking a shape is how most people select one.
    let mut harness = harness("pick-from-nothing");
    let plate = harness.state().primary().expect("the fixture puts a plate in");
    harness.state_mut().clear_selection();
    harness.step();
    assert!(harness.state().selection.is_empty());

    let at = harness.state().viewport_rect.center();
    press(&mut harness, at);
    release(&mut harness, at);

    assert_eq!(harness.state().selection, vec![plate], "clicking the shape did not select it");
}

#[test]
fn a_press_on_a_move_handle_grabs_it_even_though_the_pointer_leaves_it() {
    // A handle is grabbable within 9 px; egui does not call a press a drag
    // until the pointer has moved about 6. Deciding what was grabbed from where
    // the pointer is by then finds nothing much of the time -- which is why
    // moving an object took several attempts. What was under the *press* is what
    // was grabbed.
    let mut harness = harness("grab-slip");
    let plate = harness.state().primary().unwrap();
    harness.state_mut().mode = crate::gizmo::Mode::Move;
    harness.step();

    // Where the X arrow is drawn right now, from the gizmo itself.
    let view = harness.state().current_view();
    let gizmo = harness.state().gizmo_for(plate).expect("a gizmo for the selected plate");
    let handle = crate::gizmo::Handle::MoveAxis(0);
    let (at, _) = view.project(gizmo.handle_point(handle, &view)).expect("the X arrow is off screen");

    // Straight down the arrow, past the grab radius on the very first move --
    // the one gesture the old code could not see.
    let along = view.project(gizmo.handle_point(handle, &view) + gizmo.axes[0] * 30.0).unwrap().0;
    let steps = harness.state().history.undo_len();

    press(&mut harness, at);
    move_to(&mut harness, at + (along - at).normalized() * 12.0);
    assert!(harness.state().drag.is_some(), "the press on the arrow did not start a drag");
    move_to(&mut harness, along);
    release(&mut harness, along);

    let moved = harness.state().scene.node(plate).position;
    assert!(moved.x.abs() > 1e-9, "the drag on the X arrow moved the plate nowhere: {moved:?}");
    assert!(moved.y.abs() < 1e-9 && moved.z.abs() < 1e-9, "an X drag moved the other axes too: {moved:?}");
    assert_eq!(harness.state().history.undo_len(), steps + 1, "the drag left more than one thing to undo");
    assert_eq!(harness.state().selection, vec![plate], "grabbing a handle also re-picked what is behind it");
}

// -- the view cube ------------------------------------------------------------

/// Where the cube's front face is drawn *right now*, from the cube's own
/// projection -- the same function the drawing uses, so a test clicks what is
/// on screen rather than where it assumes it to be.
#[track_caller]
fn front_face(harness: &Harness<'_, App>) -> egui::Pos2 {
    let camera = harness.state().scene.camera;
    let cube = rect_of(harness, crate::panel_viewport::cube_id());
    let reach = crate::theme::metric::VIEW_CUBE * 0.30;
    let (normal, _, _) = crate::view::CUBE_FACES[front_index()];
    let n = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
    let (offset, depth) = crate::view::cube_project(camera.yaw, camera.pitch, n, reach);
    assert!(depth < 0.0, "the front face is turned away in this view; a click there would go through it");
    cube.center() + offset
}

fn front_index() -> usize {
    crate::view::CUBE_FACES.iter().position(|(_, _, label)| *label == "FRT").unwrap()
}

#[test]
fn clicking_a_face_of_the_view_cube_asks_for_the_view_that_face_shows() {
    let mut harness = harness("cube");
    let camera = harness.state().scene.camera;
    let preset = crate::view::CUBE_FACES[front_index()].1;
    let at = front_face(&harness);

    press(&mut harness, at);
    release(&mut harness, at);

    // The turn is animated over 200 ms, so what a click produces is a request:
    // where it ends up is what this asserts, without waiting for wall clock.
    let asked = harness.state().camera_move.expect("clicking a face asked for nothing");
    let (yaw, pitch) = preset.angles();
    assert!(
        ((asked.to.0 - yaw) / 360.0).round() * 360.0 - (asked.to.0 - yaw) < 1e-6,
        "the camera was asked for yaw {} rather than {yaw}",
        asked.to.0
    );
    assert!((asked.to.1 - pitch).abs() < 1e-6, "the camera was asked for pitch {} rather than {pitch}", asked.to.1);
    assert_eq!(asked.from, (camera.yaw, camera.pitch), "the turn did not start from where the camera was");
}

#[test]
fn the_cube_takes_the_pointer_so_a_click_on_it_does_nothing_to_the_scene_behind_it() {
    // The cube sits inside the viewport, and the viewport does two things with a
    // press of its own: it orbits, and -- on a click that grabs no handle -- it
    // selects whatever is under the pointer, or clears the selection when that
    // is nothing. Under the cube there is nothing. So a click on a face must be
    // a click on the cube only.
    let mut harness = harness("cube-orbit");
    let before = harness.state().scene.camera;
    let selected = harness.state().selection.clone();
    assert!(!selected.is_empty(), "the starting scene has its plate selected");
    let at = front_face(&harness);

    press(&mut harness, at);
    // A real click is never perfectly still; a pixel is well under egui's drag
    // threshold and must stay a click.
    move_to(&mut harness, at + egui::vec2(1.0, 1.0));
    release(&mut harness, at + egui::vec2(1.0, 1.0));

    assert!(harness.state().camera_move.is_some(), "the click did not reach the cube at all");
    assert_eq!(harness.state().selection, selected, "the click went through the cube and changed the selection");
    let after = harness.state().scene.camera;
    assert_eq!(
        (after.yaw, after.pitch),
        (before.yaw, before.pitch),
        "the click orbited the camera as well as being a click on the cube"
    );
}

// -- the 3D cursor ------------------------------------------------------------

#[test]
fn shift_right_click_puts_the_3d_cursor_where_the_pointer_is() {
    let mut harness = harness("cursor");
    assert!(harness.state().cursor.is_none(), "the cursor starts at the origin");
    let viewport = harness.state().viewport_rect;
    let camera = harness.state().scene.camera;

    // The middle of the viewport is the plate, which the starting scene frames.
    let at = viewport.center();
    modifiers(&mut harness, egui::Modifiers::SHIFT);
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();

    let cursor = harness.state().cursor.expect("shift+right-click placed no cursor");
    assert!(
        cursor.z >= 0.0 && cursor.z <= 4.0 + 1e-9,
        "the cursor landed at {cursor:?}, which is not on the plate under the pointer"
    );
    let snap = harness.state().move_snap();
    for component in [cursor.x, cursor.y, cursor.z] {
        assert!((component / snap).fract().abs() < 1e-6, "{cursor:?} is not on the move snap");
    }
    let after = harness.state().scene.camera;
    assert_eq!((after.yaw, after.pitch), (camera.yaw, camera.pitch), "placing the cursor orbited the camera");
}

#[test]
fn shift_right_click_on_empty_space_puts_the_3d_cursor_back_at_the_origin() {
    let mut harness = harness("cursor-reset");
    let viewport = harness.state().viewport_rect;
    modifiers(&mut harness, egui::Modifiers::SHIFT);
    let at = viewport.center();
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    assert!(harness.state().cursor.is_some());

    // Tip the camera under the ground plane and look up at it: from the
    // starting view, which looks down at the plate, every pixel of the viewport
    // meets the ground and there would be nowhere empty to click.
    harness.state_mut().scene.camera.pitch = -20.0;
    harness.step();

    // Somewhere there really is nothing: no geometry, and the ground plane
    // behind the eye rather than in front of it. The view says where that is,
    // so the test cannot be clicking at a spot that merely looks empty.
    let view = harness.state().current_view();
    let mesh = harness.state().evaluated.mesh.clone();
    let sky = (0..viewport.height() as usize)
        .map(|i| egui::pos2(viewport.center().x, viewport.top() + i as f32))
        .find(|p| {
            let (origin, dir) = view.ray(*p);
            view.ray_plane_ahead(*p, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).is_none()
                && crate::pick::ray_mesh(&mesh, origin, dir).is_none()
        })
        .expect("the whole viewport meets the ground plane; this test has nowhere empty to click");

    button(&mut harness, sky, egui::PointerButton::Secondary, true);
    button(&mut harness, sky, egui::PointerButton::Secondary, false);
    modifiers(&mut harness, egui::Modifiers::NONE);

    assert!(harness.state().cursor.is_none(), "a click on nothing did not put the cursor back at the origin");
}

// -- the outliner's context menu ----------------------------------------------

#[test]
fn right_clicking_an_unselected_outliner_row_opens_its_menu_at_the_first_press() {
    // The menu selects the row it was opened on, and that selection used to
    // renumber the row underneath it, so the menu was looked for under an id
    // nothing had drawn and closed again on the very next frame.
    let mut harness = harness("outliner-context-menu");
    let root = harness.state().scene.root();
    let cube = harness.state_mut().scene.add_primitive("box", root, 1).expect("the box is in the registry");
    harness.step();
    assert!(!harness.state().is_selected(cube), "the row this test right-clicks was already selected");

    let row = rect_of(&harness, crate::panel_outliner::row_id(cube));
    let at = egui::pos2(row.center().x, row.center().y);
    move_to(&mut harness, at);
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    // Two more frames: the menu is opened out of memory on the frame after the
    // click, and it is the frame after *that* which used to lose it.
    harness.step();
    harness.step();

    assert!(harness.state().is_selected(cube), "the right-click did not select the row it was on");
    assert!(
        egui::Popup::is_id_open(&harness.ctx, crate::panel_outliner::row_id(cube).with("popup")),
        "the first right-click on an unselected row did not leave its context menu open"
    );
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

#[test]
fn escape_abandons_a_half_typed_value_and_enter_takes_it() {
    // Escape is the way out of everything else in this application. It used to
    // be the one place it was not: egui surrenders focus on Escape, the field
    // committed on any loss of focus, and the abandoned text was written to the
    // model on the way out.
    let mut harness = harness("field-escape");
    let plate = harness.state().primary().expect("the starter shape is selected");
    let before = harness.state().scene.node(plate).position.x;

    // The field carries the name, because the field is also the grip.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Position (mm):0"));
    let at = field.center();

    replace_field(&mut harness, at, "40");
    key(&mut harness, egui::Key::Escape);
    harness.step();
    assert_eq!(
        harness.state().scene.node(plate).position.x,
        before,
        "Escape committed the value that was being abandoned"
    );

    // The same typing, ended with Enter, does reach the model.
    replace_field(&mut harness, at, "40");
    key(&mut harness, egui::Key::Enter);
    harness.step();
    assert_eq!(harness.state().scene.node(plate).position.x, 40.0, "Enter did not take the typed value");
}

#[test]
fn a_drag_over_the_outliner_marks_the_row_it_would_land_in_while_it_is_still_held() {
    // Issue 43. The indicator asked `hovered()`, which egui reserves for a
    // frame where nothing is being dragged -- so it was true on exactly one
    // frame of a drag, the release, and the mark nobody could see was the
    // whole complaint. Held here across several frames, which is where a
    // pointer actually is while it is looking for somewhere to drop.
    let mut harness = harness("outliner-drop-indicator");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let group = harness.state_mut().scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
    let cube = harness.state_mut().scene.add_primitive("box", root, 2).expect("the box is in the registry");
    harness.state_mut().select_only(plate);
    harness.state_mut().toggle_selected(cube);
    harness.step();
    harness.step();

    let from = rect_of(&harness, crate::panel_outliner::row_id(cube)).center();
    press(&mut harness, from);
    // A short move first, to make the press a drag.
    move_to(&mut harness, from + egui::vec2(0.0, 4.0));
    harness.step();
    // Settled over several frames, because a response reports the frame it was
    // drawn in.
    for _ in 0..3 {
        let onto = rect_of(&harness, crate::panel_outliner::row_id(group)).center();
        move_to(&mut harness, onto);
        harness.step();
    }
    let target = harness.state().drop_target.expect("nothing was marked as the drop target mid-drag");
    assert_eq!(target.into, Some(group), "the group under the pointer was not marked as what would take the drop");

    let onto = rect_of(&harness, crate::panel_outliner::row_id(group)).center();
    release(&mut harness, onto);
    harness.step();
    let app = harness.state();
    assert_eq!(app.scene.node(group).children, vec![plate, cube], "the whole selection did not land in the group");
    assert!(app.drop_target.is_none(), "the drop target outlived the drag");
}

#[test]
fn the_rows_a_drag_carries_stay_where_they_are_while_it_is_held() {
    // Issue 46. Taking the carried rows out of the tree re-flowed everything
    // below them the instant the drag began, so the gap the drop line pointed
    // at slid out from under the pointer that was aiming at it. They stay,
    // drawn faintly, and the tree does not move.
    let mut harness = harness("outliner-drag-shadow");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let cube = harness.state_mut().scene.add_primitive("box", root, 1).expect("the box is in the registry");
    let last = harness.state_mut().scene.add_primitive("sphere", root, 2).expect("the sphere is in the registry");
    harness.state_mut().select_only(plate);
    harness.state_mut().toggle_selected(cube);
    harness.step();
    harness.step();

    let carried = rect_of(&harness, crate::panel_outliner::row_id(cube));
    let below = rect_of(&harness, crate::panel_outliner::row_id(last));

    press(&mut harness, carried.center());
    move_to(&mut harness, carried.center() + egui::vec2(0.0, 4.0));
    // Settled over several frames: a response reports the frame it was drawn
    // in, so the press only becomes a drag a frame or two after the move.
    for _ in 0..4 {
        harness.step();
    }
    assert!(harness.state().outliner_drag.is_some(), "the drag never started");
    // Both carried rows are still drawn, in the same places, and so is the row
    // under them.
    assert_eq!(rect_of(&harness, crate::panel_outliner::row_id(cube)), carried, "the dragged row left the tree");
    assert_eq!(rect_of(&harness, crate::panel_outliner::row_id(plate)).height(), carried.height());
    assert_eq!(rect_of(&harness, crate::panel_outliner::row_id(last)), below, "the tree re-flowed under the pointer");

    // Let go over nothing: the drag is off and every row is back to normal.
    release(&mut harness, carried.center() + egui::vec2(0.0, 4.0));
    harness.step();
    assert!(harness.state().outliner_drag.is_none(), "the drag outlived the release");
    assert_eq!(rect_of(&harness, crate::panel_outliner::row_id(last)), below);
}

#[test]
fn a_narrow_properties_panel_stacks_its_rows_instead_of_overflowing() {
    // Issue 51. At the dock's narrowest, a label column plus three fields does
    // not fit across the panel: the fields used to shrink to slivers and the
    // last one went over the edge. The rows stack instead -- name above, one
    // axis per line -- and everything stays inside the panel.
    let mut harness = harness_configured("properties-narrow", |app| app.settings.properties_width = 200.0);
    let grips: Vec<egui::Rect> = (0..3)
        .map(|axis| rect_of(&harness, crate::panel_properties::grip_id(&format!("Position (mm):{axis}"))))
        .collect();
    // One to a line, all at the same x: three columns would put them side by
    // side at the same y.
    for pair in grips.windows(2) {
        assert!(pair[1].top() > pair[0].top(), "the axis fields are still laid out across the row: {grips:?}");
        assert!((pair[1].left() - pair[0].left()).abs() < 1.0, "the stacked fields do not line up: {grips:?}");
    }
    // And none of them is past the panel's right-hand edge.
    let right = harness.state().settings.properties_width;
    let screen = harness.ctx.screen_rect().right();
    for grip in &grips {
        assert!(grip.right() <= screen - 4.0, "a field ran off the screen: {grip:?}");
        assert!(grip.width() >= 44.0, "a field was squeezed below being readable: {grip:?} in {right}");
    }
    harness.step();
}

#[test]
fn a_long_name_widens_the_outliner_rows_instead_of_wrapping() {
    // Issue 50. A row is a fixed 22 px, so a name broken over two lines is a
    // name with its second half cut off. The rows are made as wide as the
    // longest one and the tree scrolls sideways to reach it.
    let mut harness = harness("outliner-long-name");
    let plate = harness.state().primary().expect("the starter shape is selected");
    harness.step();
    let narrow = rect_of(&harness, crate::panel_outliner::row_id(plate));

    let long = "a name far longer than any outliner panel is ever going to be wide, by some way";
    harness.state_mut().scene.get_mut(plate).unwrap().name = long.to_string();
    harness.step();
    harness.step();
    let wide = rect_of(&harness, crate::panel_outliner::row_id(plate));

    assert_eq!(wide.height(), narrow.height(), "the row grew a line instead of growing wider");
    assert!(wide.width() > narrow.width(), "the row stayed the panel's width, so the name was wrapped or cut");
}

#[test]
fn the_gap_under_an_open_group_drops_into_it_rather_than_beside_it() {
    // Issue 49. The gap under a group's row is the gap above its first child,
    // so what lands there belongs to the group -- it was landing in the
    // group's parent instead, a level out from where the line was drawn.
    let mut harness = harness("outliner-drop-under-group");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let group = harness.state_mut().scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
    let inner = harness.state_mut().scene.add_primitive("box", group, 0).expect("the box is in the registry");
    harness.state_mut().select_only(plate);
    harness.step();
    harness.step();

    let row = rect_of(&harness, crate::panel_outliner::row_id(group));
    let from = rect_of(&harness, crate::panel_outliner::row_id(plate)).center();
    press(&mut harness, from);
    move_to(&mut harness, from + egui::vec2(0.0, 4.0));
    for _ in 0..3 {
        harness.step();
    }
    assert!(harness.state().outliner_drag.is_some(), "the drag never started");

    // The bottom edge of the group's row: the gap between it and its first child.
    let at = egui::pos2(row.center().x, row.bottom() - 1.0);
    move_to(&mut harness, at);
    harness.step();
    let target = harness.state().drop_target.expect("no drop target under the group's row");
    assert_eq!(target.parent, group, "the gap under the group named another parent");
    assert_eq!(target.index, 0, "the drop was not the group's new first child");

    release(&mut harness, at);
    harness.step();
    assert_eq!(harness.state().scene.node(group).children, vec![plate, inner], "the drop landed outside the group");
    assert_eq!(harness.state().scene.node(root).children, vec![group], "the plate stayed in the root");
}

#[test]
fn the_gap_between_two_rows_is_one_drop_position_all_the_way_across() {
    // Issue 48. The rows are laid out with a gap between them, and a pointer in
    // that gap is claimed by the rows either side of it -- so both drew the
    // mark, one under the upper row and one over the lower one, two orange
    // lines a spacing apart for a single landing place. Checked here both ways:
    // every step across the gap names the same drop, and the height the two
    // rows would draw their line at is the same height.
    let mut harness = harness("outliner-drop-gap");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let cube = harness.state_mut().scene.add_primitive("box", root, 1).expect("the box is in the registry");
    let sphere = harness.state_mut().scene.add_primitive("sphere", root, 2).expect("the sphere is in the registry");
    harness.state_mut().select_only(sphere);
    harness.step();
    harness.step();

    let upper = rect_of(&harness, crate::panel_outliner::row_id(plate));
    let lower = rect_of(&harness, crate::panel_outliner::row_id(cube));
    let gap = lower.top() - upper.bottom();
    assert!(gap > 0.0, "the rows are drawn edge to edge, so this test proves nothing");

    let from = rect_of(&harness, crate::panel_outliner::row_id(sphere)).center();
    press(&mut harness, from);
    move_to(&mut harness, from + egui::vec2(0.0, 4.0));
    for _ in 0..3 {
        harness.step();
    }
    assert!(harness.state().outliner_drag.is_some(), "the drag never started");

    // Up across the gap in small steps, the way a pointer looking for a place
    // to land actually crosses it.
    let x = upper.center().x;
    let mut y = lower.top() + 2.0;
    while y >= upper.bottom() - 2.0 {
        move_to(&mut harness, egui::pos2(x, y));
        harness.step();
        let target = harness.state().drop_target.unwrap_or_else(|| panic!("no drop target at y {y}"));
        assert_eq!(target.into, None, "the gap at y {y} was read as a drop into a node");
        assert_eq!(target.parent, root, "the gap at y {y} named another parent");
        assert_eq!(target.index, 1, "the gap at y {y} named another landing place");
        y -= gap / 4.0;
    }

    // Whichever of the two rows draws it, the line lands in the middle of the
    // gap rather than on that row's own edge, so there is only ever one of it.
    let from_above = crate::panel_outliner::gap_line_y(upper, gap, true);
    let from_below = crate::panel_outliner::gap_line_y(lower, gap, false);
    assert_eq!(from_above, from_below, "the two rows either side of the gap drew the drop line at two heights");
    assert!(from_above > upper.bottom() && from_above < lower.top(), "the line is not in the gap");

    release(&mut harness, egui::pos2(x, lower.top()));
    harness.step();
    assert_eq!(harness.state().scene.node(root).children, vec![plate, sphere, cube], "the drop landed elsewhere");
}

// -- the outliner: selecting, and the rename that is not a selection ----------

/// Three rows under the root, and the harness that shows them, for the
/// selection gestures below. Returns the ids in the order the tree draws them.
fn outliner_harness(name: &str) -> (Harness<'static, App>, Vec<simple3d_core::scene::NodeId>) {
    let mut ids = Vec::new();
    let harness = harness_stepping(name, 1.0 / 60.0, |app| {
        let root = app.scene.root();
        for (index, type_id) in ["box", "sphere", "cylinder"].iter().enumerate() {
            ids.push(app.scene.add_primitive(type_id, root, index + 1).expect("the primitive is in the registry"));
        }
        app.clear_selection();
    });
    // The plate the shared harness adds is the first row; these three follow it.
    (harness, ids)
}

/// Let egui's double-click window go by, so the next click starts a fresh one
/// rather than continuing the run before it. Time in the harness moves one
/// frame at a time, so waiting is stepping.
fn wait_out_the_double_click_window(harness: &mut Harness<'_, App>) {
    // The clock egui classifies clicks against is its own, not the raw input's
    // (the harness leaves that unset), and it moves a predicted frame at a
    // time. A second of it is well past egui's 0.3 s window.
    let now = |harness: &Harness<'_, App>| harness.ctx.input(|i| i.time);
    let start = now(harness);
    let mut frames = 0;
    while now(harness) - start < 1.0 {
        harness.step();
        frames += 1;
        assert!(frames < 10_000, "the harness's clock is not moving");
    }
}

fn click_row(harness: &mut Harness<'_, App>, id: simple3d_core::scene::NodeId) {
    let row = rect_of(harness, crate::panel_outliner::row_id(id));
    let at = egui::pos2(row.left() + row.width() * 0.4, row.center().y);
    press(harness, at);
    release(harness, at);
}

#[test]
fn a_click_on_one_row_after_a_click_on_another_selects_it_and_does_not_rename_it() {
    // Issue 59. egui decides a double click from the delay between two clicks
    // alone -- neither the position nor the widget comes into it -- so clicking
    // down a list quickly opened the rename field on whichever row was clicked
    // second, on what is a single click as far as anyone using it is concerned.
    let (mut harness, ids) = outliner_harness("outliner-select-rename");

    click_row(&mut harness, ids[0]);
    assert_eq!(harness.state().selection, vec![ids[0]]);
    assert!(harness.state().rename.is_none(), "one click opened a rename");

    // Immediately afterwards, within egui's double-click delay, on another row.
    click_row(&mut harness, ids[1]);
    assert_eq!(harness.state().selection, vec![ids[1]], "the second row was not selected");
    assert!(harness.state().rename.is_none(), "a click on a second row opened a rename on it");

    // And a real double click, both halves of it on the same row, still does.
    // After a pause, or egui reads it as the third click of a run.
    wait_out_the_double_click_window(&mut harness);
    click_row(&mut harness, ids[1]);
    assert!(harness.state().rename.is_none(), "the first click of the pair opened a rename on its own");
    click_row(&mut harness, ids[1]);
    let (renaming, buffer) = harness.state().rename.clone().expect("a double click on one row did not open a rename");
    assert_eq!(renaming, ids[1]);
    assert_eq!(buffer, harness.state().scene.node(ids[1]).name);
}

#[test]
fn shift_selects_the_range_and_ctrl_adds_and_removes_one_row() {
    // Issue 60: the outliner had one selection mode -- a plain click replaced
    // the selection and any modifier at all toggled a single row, so a range
    // could only be built one Ctrl+click at a time.
    let (mut harness, ids) = outliner_harness("outliner-modifiers");
    let rows = crate::panel_outliner::visible_rows(harness.state());
    let plate = rows[1];
    assert_eq!(rows[2..], ids[..], "this test assumes the plate and then the three shapes");

    click_row(&mut harness, plate);
    assert_eq!(harness.state().selection, vec![plate]);

    // Shift takes everything from the anchor to the row clicked, in the order
    // the tree shows them, and nothing else.
    modifiers(&mut harness, egui::Modifiers::SHIFT);
    click_row(&mut harness, ids[1]);
    assert_eq!(harness.state().selection, vec![plate, ids[0], ids[1]], "shift did not select the range");
    assert!(harness.state().rename.is_none(), "a shift-click opened a rename");

    // Shift again re-measures from the same anchor rather than piling ranges
    // up: shortening the range takes rows back out of it.
    click_row(&mut harness, ids[0]);
    assert_eq!(harness.state().selection, vec![plate, ids[0]], "shift did not re-measure from the anchor");

    // Ctrl adds one row without disturbing the rest...
    modifiers(&mut harness, egui::Modifiers::COMMAND);
    click_row(&mut harness, ids[2]);
    assert_eq!(harness.state().selection, vec![plate, ids[0], ids[2]], "ctrl did not add the row");
    // ...and takes it out again.
    click_row(&mut harness, ids[2]);
    assert_eq!(harness.state().selection, vec![plate, ids[0]], "ctrl did not remove the row");

    // A plain click starts again from one row, which is the third mode.
    modifiers(&mut harness, egui::Modifiers::NONE);
    click_row(&mut harness, ids[2]);
    assert_eq!(harness.state().selection, vec![ids[2]], "a plain click did not replace the selection");
}

#[test]
fn a_shift_range_never_reaches_into_a_collapsed_group() {
    // The range runs over the rows on screen. A group that is shut has no rows
    // to select, and picking up its contents invisibly is exactly the surprise
    // a collapsible tree exists to avoid.
    let mut inner = Vec::new();
    let mut group = None;
    let mut tail = None;
    let mut harness = harness_stepping("outliner-range-collapsed", 1.0 / 60.0, |app| {
        let root = app.scene.root();
        let g = app.scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
        for index in 0..2 {
            inner.push(app.scene.add_primitive("box", g, index).expect("the box is in the registry"));
        }
        tail = Some(app.scene.add_primitive("sphere", root, 2).expect("the sphere is in the registry"));
        app.collapsed.insert(g);
        group = Some(g);
        app.clear_selection();
    });
    let (group, tail) = (group.unwrap(), tail.unwrap());
    let rows = crate::panel_outliner::visible_rows(harness.state());
    assert!(!rows.contains(&inner[0]), "the group is not collapsed, so this test proves nothing");

    click_row(&mut harness, group);
    modifiers(&mut harness, egui::Modifiers::SHIFT);
    click_row(&mut harness, tail);
    assert_eq!(
        harness.state().selection,
        vec![group, tail],
        "the range reached into a collapsed group, or missed a row it should have taken"
    );
}

#[test]
fn every_value_field_stays_inside_the_properties_panel_at_any_width() {
    // Issue 57. A wrapped horizontal ui reports `available_width` as the width
    // a *new* line would have, not what is left on the line being laid out --
    // so a field sized by it began after the label column and still asked for
    // the whole row. At the width this panel is usually dragged to, the
    // dimension fields ran 56 px past the window's edge and the Z column of
    // every transform row sat under it: the numbers a project is measured in
    // were off the screen.
    //
    // Checked across the widths the dock can be dragged through, because the
    // one that overflowed was not the narrowest -- the narrow ones stack, and
    // it was the wide ones that ran over.
    for width in [200.0_f32, 320.0, 420.0, 522.0, 700.0] {
        let harness = harness_configured("properties-width", |app| app.settings.properties_width = width);
        let edge = harness.ctx.screen_rect().right();
        let mut fields: Vec<(String, egui::Rect)> = Vec::new();
        for axis in 0..3 {
            for row in ["Position (mm)", "Rotation (deg)", "Scale (x)"] {
                let name = format!("{row}:{axis}");
                fields.push((name.clone(), rect_of(&harness, crate::panel_properties::grip_id(&name))));
            }
        }
        // The starter shape is a plate: its own three dimensions, and the
        // longest label of the panel among them.
        for name in ["Width (X)", "Depth (Y)", "Thickness (Z)", "Corner radius (0 = square)"] {
            fields.push((name.to_string(), rect_of(&harness, crate::panel_properties::grip_id(name))));
        }
        for (name, rect) in &fields {
            assert!(
                rect.right() <= edge - 4.0,
                "at a panel {width} px wide, {name} reaches {} with the window edge at {edge}",
                rect.right()
            );
            assert!(rect.left() >= 0.0 && rect.width() >= 40.0, "{name} at {width} px is {rect:?}");
        }
        // A dimension field has to leave its unit room on the same line: the
        // suffix used to be pushed onto a line of its own under the label,
        // which is the same overflow seen from the other side.
        let dimension = fields.iter().find(|(name, _)| name == "Width (X)").map(|(_, rect)| *rect).unwrap();
        let position = fields.iter().find(|(name, _)| name == "Position (mm):2").map(|(_, rect)| *rect).unwrap();
        assert!(
            dimension.right() < position.right() - 8.0,
            "the dimension field at {width} px left no room for its unit: {dimension:?} against {position:?}"
        );
    }
}

// -- issue 77: a modifier on its own is a binding ------------------------------

#[test]
fn a_modifier_released_on_its_own_fires_what_it_is_bound_to() {
    // Issue 77: Ctrl, Shift and Alt could not be bound at all, because a chord
    // needed a key beside them. Driven through the window because that is where
    // the rule lives: the toolkit reports no key event for a modifier, so the
    // press has to be read off the modifier state frame by frame.
    let mut harness = harness("modifier-only-binding");
    harness
        .state_mut()
        .keymap
        .set(
            simple3d_core::keymap::Command::ToggleGrid,
            simple3d_core::keymap::Chord::modifiers(false, false, true),
            true,
        )
        .unwrap();
    let before = harness.state().scene.settings.grid_visible;

    // Alt down for a couple of frames, then up with nothing pressed under it.
    modifiers(&mut harness, egui::Modifiers::ALT);
    harness.step();
    harness.step();
    assert_eq!(harness.state().scene.settings.grid_visible, before, "the binding fired while the key was still down");
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();
    assert_ne!(harness.state().scene.settings.grid_visible, before, "releasing the modifier did not fire its binding");

    // The same modifier held under another key is a combination, and firing that
    // must not also fire the modifier's own binding on the way out.
    let grid = harness.state().scene.settings.grid_visible;
    modifiers(&mut harness, egui::Modifiers::ALT);
    key(&mut harness, egui::Key::J);
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();
    assert_eq!(harness.state().scene.settings.grid_visible, grid, "a combination fired the modifier binding as well");
}

#[test]
fn several_keys_held_together_fire_the_combination_they_make() {
    // Any key can be the base of a combination, not only a modifier: Q+W+E is a
    // binding, and it fires on the press that completes it.
    let mut harness = harness("combination-binding");
    harness
        .state_mut()
        .keymap
        .set(simple3d_core::keymap::Command::ToggleGrid, simple3d_core::keymap::Chord::combo(["Q", "W", "E"]), true)
        .unwrap();
    let before = harness.state().scene.settings.grid_visible;

    // Held down one after another, the way a hand performs it. `keys_down` is
    // raw-input state, so each key stays down until it is let go.
    hold_key(&mut harness, egui::Key::Q, true);
    hold_key(&mut harness, egui::Key::W, true);
    assert_eq!(harness.state().scene.settings.grid_visible, before, "an incomplete combination fired");
    hold_key(&mut harness, egui::Key::E, true);
    assert_ne!(harness.state().scene.settings.grid_visible, before, "the completing press did not fire it");

    let after = harness.state().scene.settings.grid_visible;
    for key in [egui::Key::Q, egui::Key::W, egui::Key::E] {
        hold_key(&mut harness, key, false);
    }
    assert_eq!(harness.state().scene.settings.grid_visible, after, "letting go fired it a second time");
}

#[test]
fn a_modifier_held_over_a_click_is_the_click_not_a_binding() {
    // Ctrl+click adds to the selection, and must not also fire whatever Ctrl
    // alone is bound to when the hand comes off the key.
    let mut harness = harness("modifier-only-click");
    harness
        .state_mut()
        .keymap
        .set(
            simple3d_core::keymap::Command::ToggleGrid,
            simple3d_core::keymap::Chord::modifiers(true, false, false),
            true,
        )
        .unwrap();
    let before = harness.state().scene.settings.grid_visible;

    let viewport = harness.state().viewport_rect.center();
    modifiers(&mut harness, egui::Modifiers::COMMAND);
    press(&mut harness, viewport);
    release(&mut harness, viewport);
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();
    assert_eq!(harness.state().scene.settings.grid_visible, before, "a Ctrl+click also fired the Ctrl binding");
}

// -- issue 78: the measure tool's own section in the property panel ------------

#[test]
fn the_measure_section_is_there_only_while_the_tool_is_out() {
    use egui_kittest::kittest::Queryable;

    // Issue 78: the span belongs in the property panel as numbers that can be
    // typed -- and nowhere at all once the tool is put away. The section is
    // found by the one control only it has; a label the panel merely draws is
    // not in the accessibility tree to ask about.
    let mut harness = harness("measure-section");
    assert!(harness.query_by_label("Put the tool away").is_none(), "the section was there with the tool put away");

    harness.state_mut().run(simple3d_core::keymap::Command::MeasureTool);
    harness.step();
    harness.step();
    assert!(harness.query_by_label("Put the tool away").is_some(), "the tool is out and its section is not");

    // The section is wired to the tool, not just drawn beside it.
    harness.get_by_label("Put the tool away").click();
    harness.step();
    harness.step();
    assert!(!harness.state().measure.active, "the section's own button did not put the tool away");
    assert!(harness.query_by_label("Put the tool away").is_none(), "the section outlived the tool");
}

#[test]
fn a_right_click_in_the_viewport_takes_the_last_measure_point_back() {
    let mut harness = harness("measure-unplace");
    harness.state_mut().run(simple3d_core::keymap::Command::MeasureTool);
    harness.step();

    // Place both ends by clicking the viewport, the way the tool is used.
    let centre = harness.state().viewport_rect.center();
    let (a, b) = (centre - egui::vec2(60.0, 30.0), centre + egui::vec2(60.0, 30.0));
    press(&mut harness, a);
    release(&mut harness, a);
    press(&mut harness, b);
    release(&mut harness, b);
    assert_eq!(harness.state().measure.points.len(), 2, "the two clicks did not place two ends");

    // A right-click takes the last one off, and the next takes the other.
    button(&mut harness, b, egui::PointerButton::Secondary, true);
    button(&mut harness, b, egui::PointerButton::Secondary, false);
    harness.step();
    assert_eq!(harness.state().measure.points.len(), 1, "the right-click did not take the end back");
    button(&mut harness, b, egui::PointerButton::Secondary, true);
    button(&mut harness, b, egui::PointerButton::Secondary, false);
    harness.step();
    assert!(harness.state().measure.points.is_empty(), "the start is still placed");
    assert!(harness.state().measure.active, "the tool was put away by a right-click");
}

#[test]
fn the_measure_section_shows_the_span_and_takes_it_back() {
    use egui_kittest::kittest::Queryable;

    // The ends are editable fields, so they are spin buttons in the panel: three
    // for the start and three for the end, and they read what the tool holds.
    let mut harness = harness("measure-fields");
    harness.state_mut().run(simple3d_core::keymap::Command::MeasureTool);
    harness.state_mut().measure.set_point(0, Vec3::new(1.0, 2.0, 3.0));
    harness.state_mut().measure.set_point(1, Vec3::new(11.0, 2.0, 3.0));
    harness.step();
    harness.step();

    let shown: Vec<String> = harness
        .get_all_by_role(egui::accesskit::Role::SpinButton)
        .filter_map(|n| n.value().map(|v| v.to_string()))
        .collect();
    for expected in ["1", "2", "3", "11"] {
        assert!(shown.iter().any(|v| v == expected), "no field reads {expected}: {shown:?}");
    }

    // Clearing from the panel takes the span away without putting the tool away.
    harness.get_by_label("Clear the span").click();
    harness.step();
    assert!(harness.state().measure.points.is_empty(), "the panel's Clear left the span in place");
    assert!(harness.state().measure.active, "clearing the span also put the tool away");
}

#[test]
fn a_patterns_kind_is_clicked_from_a_row_and_its_fields_are_all_one_width() {
    use egui_kittest::kittest::Queryable;

    // Two layout changes to the pattern editor, both checked against the real
    // panel because both are about where things are drawn.
    //
    // The kind and axis options flow across their row instead of taking a line
    // each -- so the first thing to prove is that they are still *buttons* after
    // being lifted out of the vertical layout they used to sit in.
    //
    // And the unit is in the row's name, in brackets, rather than after the
    // field. That is what makes every field one width: a count carries no unit
    // and a step does, and the two used to end at different places down the same
    // column.
    let mut harness = harness_configured("pattern-panel", |app| {
        app.run(simple3d_core::keymap::Command::Pattern);
    });
    let pattern = harness.state().primary().expect("the tool leaves the pattern selected");
    let kind = |harness: &Harness<'_, App>| harness.state().scene.node(pattern).params().unwrap().int("kind");
    assert_eq!(kind(&harness), 0, "a fresh pattern is linear");

    // Every kind is on the row, and clicking one chooses it.
    for (index, name) in ["Linear", "Grid", "Circular", "Mirror", "Helix", "Spiral"].iter().enumerate() {
        assert!(harness.query_by_label(name).is_some(), "{name} is not on the kind row");
        harness.get_by_label(name).click();
        harness.step();
        harness.step();
        assert_eq!(kind(&harness), index as u32, "clicking {name} did not choose it");
    }

    // A circular pattern is the case the width rule is about: Copies has no
    // unit, Span is in degrees and Radius in millimetres, and all three fields
    // have to start and end together.
    harness.get_by_label("Circular").click();
    harness.step();
    harness.step();
    //
    // Checked across the kinds, not just this one: the longest names a pattern
    // has are a spiral's, and "Radius per copy (mm)" is the one a bracketed unit
    // could have pushed out of the label column and into the field beside it.
    let mut fields: Vec<(&str, egui::Rect)> = Vec::new();
    for (kind, names) in [
        ("Circular", &["Copies", "Span", "Radius"][..]),
        ("Spiral", &["Copies", "Angle per copy", "Start radius", "Radius per copy", "Rise per copy"][..]),
    ] {
        harness.get_by_label(kind).click();
        harness.step();
        harness.step();
        for name in names {
            fields.push((name, rect_of(&harness, crate::panel_properties::grip_id(name))));
        }
    }
    for pair in fields.windows(2) {
        let ((a_name, a), (b_name, b)) = (pair[0], pair[1]);
        assert!(
            (a.width() - b.width()).abs() < 0.5 && (a.right() - b.right()).abs() < 0.5,
            "{a_name} and {b_name} are different sizes: {a:?} and {b:?}"
        );
    }

    harness.get_by_label("Circular").click();
    harness.step();
    harness.step();

    // And the axis options are buttons on their own row, the same as the kinds.
    for (index, name) in ["X", "Y", "Z"].iter().enumerate() {
        harness.get_by_label(name).click();
        harness.step();
        harness.step();
        let axis = harness.state().scene.node(pattern).params().unwrap().int("circ_axis");
        assert_eq!(axis, index as u32, "clicking axis {name} did not choose it");
    }
}

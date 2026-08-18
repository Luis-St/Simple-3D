//! The gestures a pointer has to perform, performed.
//!
//! Everything in here drives `App::ui` -- the same panels, in the same order,
//! that the running window draws -- against a headless context, and replays real
//! pointer events over it with `egui_kittest`. Nothing calls the arithmetic
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
use scadstudio_core::config::{Panel, Side};
use scadstudio_core::eval::{Cancel, Evaluator};
use scadstudio_core::primitive::ParamsExt;
use scadstudio_geom::Vec3;

/// An `App` on its own config directory, wired into a harness that draws one
/// real frame per step.
fn harness(name: &str) -> Harness<'static, App> {
    let dir = std::env::temp_dir().join(format!(
        "scadstudio-gesture-test-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut app = App::with_config_dir(&egui::Context::default(), None, dir);
    // The worker is asynchronous, and a test must not wait on it: evaluate the
    // starting scene here so picking, the manipulator and the 3D cursor have
    // geometry to work against.
    app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());

    let mut themed = false;
    let mut harness = Harness::builder().with_size(egui::vec2(1400.0, 880.0)).build_state(
        move |ctx, app: &mut App| {
            if !themed {
                // `App::new` themes the context it is handed; the harness makes
                // its own, so it is themed here instead. Sizes come from the
                // theme, so a frame drawn without it is not the real frame.
                crate::theme::apply(ctx);
                themed = true;
            }
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

// -- the field label scrub ----------------------------------------------------

#[test]
fn dragging_a_field_label_scrubs_its_value_in_one_undo_step() {
    let mut harness = harness("scrub");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    let width = |harness: &Harness<'_, App>| harness.state().scene.node(plate).params().unwrap().num("width");
    assert_eq!(width(&harness), 40.0);
    let steps = harness.state().history.undo_len();

    // Sixty pixels to the right at six pixels a millimetre is ten millimetres.
    let grip = rect_of(&harness, crate::panel_properties::grip_id("Width (X)"));
    drag(&mut harness, grip.center(), grip.center() + egui::vec2(60.0, 0.0), 6);

    assert!((width(&harness) - 50.0).abs() < 1e-9, "the label scrub gave {} rather than 50 mm", width(&harness));
    assert_eq!(
        harness.state().history.undo_len(),
        steps + 1,
        "a scrub across six frames left more than one thing to undo"
    );
    assert!(harness.state().scrub.id.is_none(), "the scrub was left running after the button came up");

    harness.state_mut().run(scadstudio_core::keymap::Command::Undo);
    assert_eq!(width(&harness), 40.0, "one undo did not take the whole scrub back");
}

#[test]
fn a_scrub_that_runs_off_its_label_keeps_scrubbing_that_field_and_no_other() {
    // The pointer leaves the label almost immediately -- six pixels a
    // millimetre means any real edit crosses it -- and it passes over the other
    // two dimension labels on its way. Neither may take the gesture over.
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
    assert_eq!((d, t), (20.0, 4.0), "a label the pointer merely crossed was scrubbed too");
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
            view.ray_plane(*p, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).is_none()
                && crate::pick::ray_mesh(&mesh, origin, dir).is_none()
        })
        .expect("the whole viewport meets the ground plane; this test has nowhere empty to click");

    button(&mut harness, sky, egui::PointerButton::Secondary, true);
    button(&mut harness, sky, egui::PointerButton::Secondary, false);
    modifiers(&mut harness, egui::Modifiers::NONE);

    assert!(harness.state().cursor.is_none(), "a click on nothing did not put the cursor back at the origin");
}

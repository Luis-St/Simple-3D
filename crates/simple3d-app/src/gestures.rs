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

#[test]
fn a_setting_scrubs_on_one_axis_with_the_same_modifiers_as_a_dimension() {
    // The document's own numbers -- the step, the grid, the segment default, the
    // 3D cursor, the ends of the measure span -- were egui's `DragValue`, which
    // is a different control: it counted vertical movement as a second axis to
    // edit on and it had no coarse modifier. They come through the same field as
    // every dimension now, and the step is the one that is on screen with a
    // shape selected, so it is the one dragged here.
    let mut harness = harness("scrub-setting");
    let step = |harness: &Harness<'_, App>| harness.state().scene.settings.snap_step;
    assert_eq!(step(&harness), 1.0);
    let history = harness.state().history.undo_len();

    // Straight down, which used to change the value and must now do nothing at
    // all -- including leaving a step behind that undoes nothing.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Step"));
    drag(&mut harness, field.center(), field.center() + egui::vec2(0.0, 60.0), 6);
    assert_eq!(step(&harness), 1.0, "a drag straight down the field changed the step");
    assert_eq!(harness.state().history.undo_len(), history, "a drag that changed nothing left an undo step");

    // Sixty pixels to the right at six pixels a step is ten millimetres, and the
    // whole drag is one thing to undo.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Step"));
    drag(&mut harness, field.center(), field.center() + egui::vec2(60.0, 0.0), 6);
    assert!((step(&harness) - 11.0).abs() < 1e-9, "the scrub gave {} rather than 11 mm", step(&harness));
    assert_eq!(harness.state().history.undo_len(), history + 1, "a scrub across six frames left more than one step");
    harness.state_mut().run(simple3d_core::keymap::Command::Undo);
    assert_eq!(step(&harness), 1.0, "one undo did not take the whole scrub back");

    // And the coarse modifier, which egui's control did not have: the same sixty
    // pixels are worth ten times as much with Ctrl held.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Step"));
    modifiers(&mut harness, egui::Modifiers::COMMAND);
    drag(&mut harness, field.center(), field.center() + egui::vec2(60.0, 0.0), 6);
    modifiers(&mut harness, egui::Modifiers::NONE);
    assert!((step(&harness) - 101.0).abs() < 1e-9, "a coarse scrub gave {} rather than 101 mm", step(&harness));
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

// -- navigating: the middle button moves about the ground ---------------------

/// A drag on a named button, which `drag` above cannot do: it presses the
/// primary one, because that is what every gesture before this one used.
fn drag_button(
    harness: &mut Harness<'_, App>,
    button: egui::PointerButton,
    from: egui::Pos2,
    to: egui::Pos2,
    frames: usize,
) {
    move_to(harness, from);
    self::button(harness, from, button, true);
    for frame in 1..=frames {
        let t = frame as f32 / frames as f32;
        move_to(harness, from + (to - from) * t);
    }
    self::button(harness, to, button, false);
    harness.step();
}

/// Issue 72: the viewport reads as bolted to the origin because orbit and zoom
/// both hold the camera's target still, and pan -- the one gesture that moves
/// it -- was behind Shift. It is the middle button now, and this drives the real
/// pointer through the real panel rather than calling `apply_gesture`.
#[test]
fn a_middle_drag_on_the_viewport_moves_the_camera_over_the_ground() {
    let mut harness = harness("viewport-pan");
    let rect = harness.state().viewport_rect;
    // Well clear of the view cube in the corner, which takes the pointer first.
    let from = rect.center() + egui::vec2(-120.0, 40.0);
    let to = from + egui::vec2(140.0, 90.0);
    let before = harness.state().scene.camera;

    drag_button(&mut harness, egui::PointerButton::Middle, from, to, 6);

    let after = harness.state().scene.camera;
    assert_ne!(after.target, before.target, "a middle drag did not move the camera over the ground");
    // Pan and nothing else: the picture must not have turned or zoomed with it.
    assert_eq!((after.yaw, after.pitch), (before.yaw, before.pitch), "the middle drag orbited as well");
    assert_eq!(after.distance, before.distance, "the middle drag zoomed as well");

    // It follows the pointer: dragging right and down carries the *scene* that
    // way, so the camera goes the other way along the screen's own axes.
    let view = crate::view::View::new(before, rect);
    let (right, up) = view.basis();
    let moved = after.target - before.target;
    assert!(moved.dot(right) < 0.0, "dragging right did not carry the ground right: {moved:?}");
    assert!(moved.dot(up) > 0.0, "dragging down did not carry the ground down: {moved:?}");

    // And the right button still orbits, on the same map, without a modifier.
    let turned = harness.state().scene.camera;
    drag_button(&mut harness, egui::PointerButton::Secondary, from, to, 6);
    let orbited = harness.state().scene.camera;
    assert_ne!(orbited.yaw, turned.yaw, "the right button stopped orbiting");
    assert_eq!(orbited.target, turned.target, "orbiting moved the camera over the ground");
}

/// A turn of the wheel over the viewport, as the window gets it.
fn wheel(harness: &mut Harness<'_, App>, at: egui::Pos2, delta: egui::Vec2) {
    move_to(harness, at);
    let modifiers = harness.input().modifiers;
    event(harness, egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, modifiers });
}

/// The other half of issue 72: the wheel zooms about the pointer rather than
/// about the middle of the frame, so what is under the cursor stays under it and
/// the wheel alone carries the view across the grid. Driven through the real
/// panel, wheel event and all, rather than by calling `apply_zoom`.
#[test]
fn a_scroll_over_the_viewport_zooms_about_the_pointer() {
    let mut harness = harness("viewport-zoom");
    let rect = harness.state().viewport_rect;
    // Clear of the view cube in the corner, which takes the pointer first.
    let at = rect.center() + egui::vec2(-150.0, 90.0);
    let before = harness.state().scene.camera;
    // The world point the cursor is over. Parallel projection, so any point
    // along its ray is the same pixel and this needs nothing to be there.
    let under = crate::view::View::new(before, rect).ray(at).0;

    wheel(&mut harness, at, egui::vec2(0.0, 60.0));

    let after = harness.state().scene.camera;
    assert!(
        after.distance < before.distance,
        "scrolling up did not zoom in: {} -> {}",
        before.distance,
        after.distance
    );
    assert_eq!((after.yaw, after.pitch), (before.yaw, before.pitch), "the wheel turned the camera as well");
    assert_ne!(after.target, before.target, "the zoom held the frame centre still instead of the pointer");

    let landed = crate::view::View::new(after, rect).project(under).unwrap().0;
    assert!((landed - at).length() < 1.0, "the point under the pointer slid from {at:?} to {landed:?}");
}

/// The pattern tool's preview is a viewport too, and it navigates on the very
/// same bindings -- so it moves off the origin with the same drag (issue 72).
#[test]
fn a_middle_drag_on_the_pattern_previews_picture_moves_it_over_the_ground() {
    let mut harness = harness("pattern-preview-pan");
    harness.state_mut().open_pattern_tool();
    harness.step();
    harness.step();
    assert_eq!(harness.state().modal, crate::app::Modal::PatternKind, "the tool did not open");

    let rect = rect_of(&harness, crate::pattern_tool::preview_id());
    let from = rect.center() + egui::vec2(-40.0, 0.0);
    let to = from + egui::vec2(80.0, 50.0);
    let before = harness.state().pattern_preview_camera;
    let viewport = harness.state().scene.camera;

    drag_button(&mut harness, egui::PointerButton::Middle, from, to, 6);

    let after = harness.state().pattern_preview_camera;
    assert_ne!(after.target, before.target, "a middle drag did not move the preview over the ground");
    assert_eq!((after.yaw, after.pitch), (before.yaw, before.pitch), "the preview orbited as well");
    assert_eq!(harness.state().scene.camera, viewport, "moving the preview moved the viewport behind it");
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

// -- the view centre ----------------------------------------------------------

/// The Document section says what the camera is looking at, and takes a new
/// answer.
///
/// Pan and the wheel move that point but no gesture states it, and once the view
/// has wandered there was nothing that said "back to the origin" -- Frame is the
/// nearest, and it re-frames the model rather than re-centring the view. The
/// three fields read the camera live and write it, and the button re-centres it
/// without touching the angle or the distance it looks from.
///
/// The Document section is drawn only with nothing selected, so the test clears
/// the selection first, the way reaching those rows in the running application
/// does.
#[test]
fn the_document_section_reads_and_sets_what_the_camera_looks_at() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("view-centre");
    let rect = harness.state().viewport_rect;

    // Carry the view off the origin with the middle drag a user would use.
    let from = rect.center() + egui::vec2(-120.0, 40.0);
    drag_button(&mut harness, egui::PointerButton::Middle, from, from + egui::vec2(140.0, 90.0), 6);
    let moved = harness.state().scene.camera.target;
    assert_ne!(moved, Vec3::ZERO, "the middle drag did not move the view centre");

    harness.state_mut().selection.clear();
    harness.step();

    // The field holds what the drag left, and takes a number over it.
    let field = rect_of(&harness, crate::panel_properties::grip_id("View centre:0"));
    press(&mut harness, field.center());
    release(&mut harness, field.center());
    // The frame after the click is the one that opens the text field.
    harness.step();
    text(&mut harness, "25");
    key(&mut harness, egui::Key::Enter);
    harness.step();

    let typed = harness.state().scene.camera.target;
    assert!((typed.x - 25.0).abs() < 1e-9, "typing 25 into the X field left the camera looking at {typed:?}");
    assert_eq!((typed.y, typed.z), (moved.y, moved.z), "typing the X moved the other two axes with it");

    // And the button puts it back on the origin, from the same angle and the
    // same distance -- re-centring a view is not re-framing it.
    let before = harness.state().scene.camera;
    harness.get_by_label("Reset to origin").click();
    harness.step();
    let after = harness.state().scene.camera;
    assert_eq!(after.target, Vec3::ZERO, "the button left the view centre at {:?}", after.target);
    assert_eq!(
        (after.yaw, after.pitch, after.distance),
        (before.yaw, before.pitch, before.distance),
        "re-centring the view turned or zoomed the camera as well"
    );
}

/// Locked, the view centre is the point the camera turns about and nothing
/// moves it.
///
/// It is one setting read in three places -- the panel's fields, the viewport's
/// pan and wheel, and framing -- so the test drives all three. Orbit and the
/// zoom itself must go on working: pinning the point is what they are pinned
/// *for*, and a lock that stopped the camera moving at all would be a lock on
/// the view, not on its centre.
#[test]
fn a_locked_view_centre_is_the_one_thing_that_does_not_move() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("view-centre-lock");
    let rect = harness.state().viewport_rect;
    harness.state_mut().selection.clear();
    // Off the origin to begin with, so a lock that quietly re-centred would show.
    harness.state_mut().scene.camera.target = Vec3::new(12.0, -8.0, 3.0);
    harness.step();

    harness.get_by_label("Lock").click();
    harness.step();
    assert!(harness.state().settings.lock_view_centre, "the Lock button locked nothing");

    let pinned = harness.state().scene.camera.target;
    let before = harness.state().scene.camera;
    let from = rect.center() + egui::vec2(-120.0, 40.0);

    drag_button(&mut harness, egui::PointerButton::Middle, from, from + egui::vec2(140.0, 90.0), 6);
    assert_eq!(harness.state().scene.camera.target, pinned, "a pan moved a locked view centre");

    // The wheel still zooms; what it no longer does is walk the centre towards
    // the pointer on the way.
    wheel(&mut harness, from, egui::vec2(0.0, 60.0));
    let zoomed = harness.state().scene.camera;
    assert_eq!(zoomed.target, pinned, "the wheel walked a locked view centre towards the pointer");
    assert!(zoomed.distance < before.distance, "locking the view centre stopped the wheel zooming");

    drag_button(&mut harness, egui::PointerButton::Secondary, from, from + egui::vec2(80.0, 0.0), 6);
    assert_ne!(harness.state().scene.camera.yaw, before.yaw, "locking the view centre stopped the orbit");
    assert_eq!(harness.state().scene.camera.target, pinned, "the orbit moved the locked view centre");

    // Framing is the one command whose job is to move the centre. It fits the
    // zoom and leaves the centre where it is.
    let distance = harness.state().scene.camera.distance;
    harness.state_mut().frame_all();
    harness.step();
    assert_eq!(harness.state().scene.camera.target, pinned, "framing moved a locked view centre");
    assert_ne!(harness.state().scene.camera.distance, distance, "framing did not fit the zoom either");

    // And the fields are a readout: they take no number while it is locked.
    let field = rect_of(&harness, crate::panel_properties::grip_id("View centre:0"));
    press(&mut harness, field.center());
    release(&mut harness, field.center());
    harness.step();
    text(&mut harness, "500");
    key(&mut harness, egui::Key::Enter);
    harness.step();
    assert_eq!(harness.state().scene.camera.target, pinned, "a locked field still took a number");

    // Unlocked, the pan is given back.
    harness.get_by_label("Lock").click();
    harness.step();
    assert!(!harness.state().settings.lock_view_centre, "the Lock button did not unlock");
    drag_button(&mut harness, egui::PointerButton::Middle, from, from + egui::vec2(140.0, 90.0), 6);
    assert_ne!(harness.state().scene.camera.target, pinned, "unlocking did not give the pan back");
}

/// Clicking a value field puts the whole number under the caret, so what is typed
/// next replaces it.
///
/// It opened with the caret at the end instead, so clicking a field that read 40
/// and typing 12 gave 4012. On a count with a maximum it was worse and stranger:
/// a pattern's Copies showing 7, clicked and typed "12" into, came out as 512 --
/// 712, clamped to the most copies a pattern will lay down. The field is opened
/// by a *click*, which is one gesture on the value as a whole and never a caret
/// placed anywhere in particular, so the whole value is what it opens with.
#[test]
fn clicking_a_value_field_puts_the_whole_number_under_the_caret() {
    use simple3d_core::primitive::ParamsExt;

    let mut harness = harness("field-select-on-open");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    let width =
        |h: &Harness<'_, App>| h.state().scene.node(plate).params().expect("a plate has parameters").num("width");
    assert_eq!(width(&harness), 40.0, "this test types over a 40, and the field does not hold one");

    let field = rect_of(&harness, crate::panel_properties::grip_id("Width (X)"));
    press(&mut harness, field.center());
    release(&mut harness, field.center());
    // The frame after the click is the one that draws the text field and hands it
    // the keyboard, which is also where the caret is put across the value.
    harness.step();
    text(&mut harness, "12");
    key(&mut harness, egui::Key::Enter);
    harness.step();

    assert_eq!(width(&harness), 12.0, "typing 12 over a clicked field holding 40 gave {}", width(&harness));
}

/// A count follows the pointer at the rate every other field does.
///
/// A count is stored whole, and each frame of a scrub read the stored value back
/// out of the model before adding that frame's movement to it -- so the fraction
/// of a step a single frame is worth was rounded away sixty times a second
/// rather than added up. A hand moving a pixel a frame rounded to nothing and the
/// field never moved at all; a hand moving four rounded *up* every frame and the
/// field ran away from the pointer, which is what made the pattern's numbers feel
/// unlike every other number in the application. Sixty pixels of drag put twelve
/// copies on a pattern that should have gained ten, and a slow drag across the
/// same sixty put on none.
///
/// Both halves are the same claim, so the test is one comparison: one step is one
/// copy and it is also one millimetre, so a count dragged a given distance has to
/// change by what a length field dragged the same distance changes by. The
/// pattern offers both on adjacent rows.
#[test]
fn a_count_field_follows_the_pointer_at_the_rate_every_other_field_does() {
    for (pixels, frames) in [(12.0_f32, 12usize), (60.0, 6), (60.0, 40)] {
        let mut harness = harness("count-scrub-rate");
        harness.state_mut().run(simple3d_core::keymap::Command::Pattern);
        harness.step();
        harness.step();
        let pattern = harness.state().primary().expect("the command leaves the new pattern selected");
        let value = |h: &Harness<'_, App>, key: &str| {
            use simple3d_core::primitive::ParamsExt;
            h.state().scene.node(pattern).params().expect("a pattern carries parameters").num(key)
        };
        let (copies, step) = (value(&harness, "count"), value(&harness, "step_x"));

        let count_field = rect_of(&harness, crate::panel_properties::grip_id("Copies"));
        drag(&mut harness, count_field.center(), count_field.center() + egui::vec2(pixels, 0.0), frames);
        let length_field = rect_of(&harness, crate::panel_properties::grip_id("Step X"));
        drag(&mut harness, length_field.center(), length_field.center() + egui::vec2(pixels, 0.0), frames);

        let gained = value(&harness, "count") - copies;
        let moved = value(&harness, "step_x") - step;
        assert!(gained > 0.0, "{pixels} px over {frames} frames did not move the count at all");
        // Within half a step, which is all that can be left between them once
        // the fraction is carried rather than dropped: the count is following the
        // same accumulated pointer movement, and rounding it to a whole one is
        // the only difference. Not rounded and compared exactly, because a drag
        // that lands on a half is on the rounding boundary and the two sides of
        // it disagree by a part in a hundred million million.
        const HALF: f64 = 0.5 + 1e-6;
        assert!(
            (gained - moved).abs() <= HALF,
            "{pixels} px over {frames} frames gained {gained} copies \
             where the millimetre field on the next row moved {moved}"
        );
    }
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

/// Issue 67: the tree's own Add menu offers the custom-kind tool, the way the
/// menu bar's does.
///
/// The two Add menus are meant to hold the same things and differ only in where
/// what they add lands, and they drifted: "Custom pattern" went into the menu
/// bar's only, so the tree -- which is where a container is most often added
/// from -- offered "Pattern" and "Make a pattern of the selection" and no way at
/// all to reach the tool that builds a rule. A feature nobody can find is a
/// feature nobody has.
#[test]
fn the_tree_add_menu_offers_the_custom_pattern_tool() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("outliner-add-custom-pattern");
    let root = harness.state().scene.root();
    let cube = harness.state_mut().scene.add_primitive("box", root, 1).expect("the box is in the registry");
    harness.step();

    let row = rect_of(&harness, crate::panel_outliner::row_id(cube));
    let at = row.center();
    move_to(&mut harness, at);
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    harness.step();
    harness.step();

    // The menu bar has an "Add" of its own, and it is the one a plain lookup by
    // label finds -- which is how this test first passed against the very code it
    // was written to catch. The tree's is a submenu, so its label carries the
    // arrow egui puts on one, and it is drawn well below the menu bar.
    let add = harness
        .query_all_by_label_contains("Add")
        .find(|node| node.rect().center().y > 40.0)
        .expect("the tree's context menu has no Add of its own");
    add.click();
    harness.step();
    harness.step();
    assert!(
        harness.query_by_label("Custom pattern").is_some(),
        "the tree's Add menu has no way through to the custom pattern kind tool"
    );

    harness.get_by_label("Custom pattern").click();
    harness.step();
    harness.step();
    assert_eq!(harness.state().modal, crate::app::Modal::PatternKind, "the entry did not open the tool");
    let opened = harness.state().pattern_tool.expect("the tool opened on nothing");
    assert!(harness.state().scene.node(opened).is_pattern(), "the tool opened on something that is not a pattern");
}

#[test]
fn a_custom_pattern_offers_the_tool_in_place_of_its_stages() {
    use egui_kittest::kittest::Queryable;

    // Asked for from the running application. A custom kind's rule is thirty-odd
    // numbered fields -- "1 Copies", "3 Radius per copy" -- and the Pattern
    // section listed every one of them under the kind row, where they say
    // nothing about the rule they make and are only a wall to scroll past on the
    // way to the button that opens the tool. The tool is where they are edited,
    // and it draws each stage beside what it lays down.
    let mut harness = harness("pattern-custom-rows");
    harness.state_mut().add_pattern();
    harness.step();
    let id = harness.state().primary().expect("the new pattern is selected");

    // A fixed kind still shows its numbers: this is about the custom one only.
    assert!(
        harness.ctx.read_response(crate::panel_properties::grip_id("Copies")).is_some(),
        "a linear pattern lost the numbers that place it"
    );
    // And it is offered no way into the tool. The button used to sit under every
    // kind reading "Custom kind...", which made becoming custom something that
    // happened on the way to opening a tool rather than a kind the user picked:
    // the Kind row is where that is said.
    assert!(harness.query_by_label("Edit kind").is_none(), "a linear pattern offers the custom kind's tool");

    let custom = simple3d_core::pattern::CUSTOM;
    harness
        .state_mut()
        .scene
        .get_mut(id)
        .and_then(|node| node.params_mut())
        .expect("a pattern has parameters")
        .insert("kind".to_string(), simple3d_core::primitive::ParamValue::Choice(custom));
    harness.step();

    // The stages are gone, all four of them.
    for stage in 0..simple3d_core::pattern::MAX_STAGES {
        for key in simple3d_core::pattern::stage_keys(stage) {
            let spec = simple3d_core::pattern::PARAMS.iter().find(|p| p.key == key).expect("a stage key with no spec");
            assert!(
                harness.ctx.read_response(crate::panel_properties::grip_id(spec.label)).is_none(),
                "the Pattern section still draws {}",
                spec.label
            );
        }
    }
    assert!(
        harness.ctx.read_response(crate::panel_properties::grip_id("Stages")).is_none(),
        "the Pattern section still draws the stage count"
    );
    // What is left is the way in to the tool, the kind itself, and the line
    // saying what the pattern makes.
    assert!(harness.query_by_label("Edit kind").is_some(), "the way into the tool went with the stages");
    assert!(harness.query_by_label("Custom").is_some(), "the kind row went with the stages");
    assert!(
        harness.query_by_label_contains("Put shapes under this pattern").is_some(),
        "the line saying what the pattern makes went with the stages"
    );
}

#[test]
fn a_custom_pattern_picks_a_saved_kind_from_the_panel() {
    use egui_kittest::kittest::Queryable;

    // Asked for from the running application: a kind saved from the tool is
    // meant to be used again, and reaching one should not mean opening the tool
    // -- picking it is a single click on a name. The shelf the tool keeps is
    // offered on the pattern itself, beside the button that edits it.
    let mut harness = harness("pattern-saved-kind-panel");
    harness.state_mut().add_pattern();
    harness.step();
    let id = harness.state().primary().expect("the new pattern is selected");

    // A rule on the shelf: two stages, twelve copies -- the same shape as the
    // one the tool's own test saves, and nothing a fixed kind lays out.
    let mut rule = simple3d_core::pattern::default_params();
    rule.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(2));
    rule.insert("stage1_count".to_string(), simple3d_core::primitive::ParamValue::Count(3));
    rule.insert("stage2_count".to_string(), simple3d_core::primitive::ParamValue::Count(4));
    rule.insert("stage2_turn".to_string(), simple3d_core::primitive::ParamValue::Angle(90.0));
    let config = harness.state().config_dir().to_path_buf();
    simple3d_core::pattern_library::save(&config, "Turned row", &rule).expect("the shelf could not be written");
    harness.state_mut().refresh_pattern_kinds();

    // The shelf is offered on a custom pattern only: a linear one has a kind of
    // its own and its numbers to say it.
    harness.step();
    assert!(
        harness.ctx.read_response(crate::panel_properties::saved_kind_id()).is_none(),
        "a linear pattern offers the custom shelf"
    );

    harness
        .state_mut()
        .scene
        .get_mut(id)
        .and_then(|node| node.params_mut())
        .expect("a pattern has parameters")
        .insert("kind".to_string(), simple3d_core::primitive::ParamValue::Choice(simple3d_core::pattern::CUSTOM));
    harness.step();

    // Nothing is picked yet, so the box says so rather than naming a kind.
    let box_rect = rect_of(&harness, crate::panel_properties::saved_kind_id());
    press(&mut harness, box_rect.center());
    release(&mut harness, box_rect.center());
    harness.step();
    harness.step();
    harness.get_by_label("Turned row").click();
    harness.step();
    harness.step();

    let applied = harness.state().scene.node(id).params().cloned().expect("a pattern has parameters");
    assert_eq!(applied.int("kind"), simple3d_core::pattern::CUSTOM);
    assert_eq!(
        simple3d_core::pattern::instance_count(&applied).1,
        12,
        "the saved rule was not put on the pattern: {applied:?}"
    );
    // And the pattern is named after what was picked, which is what the box
    // reads back -- the same thing the tool's own shelf shows.
    assert_eq!(harness.state().scene.node(id).name, "Turned row", "the pattern was not named after the kind");
    assert!(
        harness.ctx.read_response(crate::panel_properties::saved_kind_id()).is_some(),
        "the box went once a kind was on the pattern"
    );
}

/// The Frame button puts the preview back the way it opened -- the angle and
/// the zoom with the rest, not just where the camera is pointed.
///
/// Asked for after the button was found to re-centre an orbited picture and
/// leave it orbited, which is half a reset and reads as a button that half
/// works. The automatic reframing, when a rule starts laying its copies out
/// somewhere else, still leaves the angle alone: that one happens under the
/// user's hands, and turning the picture mid-orbit is not what they asked for.
///
/// Driven through the button itself rather than through
/// `App::reset_pattern_preview`, because what is being checked is the wiring:
/// the dialog is `Embedded` against a headless context, so the tool is drawn
/// where the harness can click it.
#[test]
fn the_pattern_tools_frame_button_puts_the_angle_and_the_zoom_back() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("pattern-preview-frame");
    harness.state_mut().open_pattern_tool();
    harness.step();
    harness.step();
    assert_eq!(harness.state().modal, crate::app::Modal::PatternKind, "the tool did not open");

    // The picture opens at the viewport's angle. Turn it away and pull it out,
    // the way looking round a pattern does.
    let opened = harness.state().pattern_preview_camera;
    assert_eq!(opened.yaw, harness.state().scene.camera.yaw, "the preview did not open at the viewport's angle");
    {
        let camera = &mut harness.state_mut().pattern_preview_camera;
        camera.yaw += 73.0;
        camera.pitch += 21.0;
        camera.distance *= 4.0;
    }
    harness.step();

    harness.get_by_label("Frame").click();
    harness.step();
    harness.step();

    let now = harness.state().pattern_preview_camera;
    let viewport = harness.state().scene.camera;
    assert_eq!(now.yaw, viewport.yaw, "Frame left the picture turned away from the viewport's angle");
    assert_eq!(now.pitch, viewport.pitch, "Frame left the picture pitched away from the viewport's angle");
    assert!(
        (now.distance - opened.distance).abs() < 1e-6,
        "Frame left the picture at {} rather than the {} it opened at",
        now.distance,
        opened.distance
    );
    assert_eq!(harness.state().scene.camera, viewport, "framing the preview moved the viewport behind it");
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
fn the_clipboard_chords_reach_the_application() {
    // `egui-winit` turns Ctrl+X, Ctrl+C and Ctrl+V into `Cut`, `Copy` and
    // `Paste` and never emits the key press underneath them, so these three
    // bindings -- alone in the whole keymap -- can only be tested through the
    // events the window system really delivers. Driving `Command::Copy`
    // directly, which is what the other tests do, is exactly what let this go
    // unnoticed.
    let mut harness = harness("clipboard-chords");
    let root = harness.state().scene.root();
    let before = harness.state().scene.node(root).children.len();

    event(&mut harness, egui::Event::Copy);
    assert!(harness.state().clipboard.is_some(), "Ctrl+C filled the clipboard");

    event(&mut harness, egui::Event::Paste("anything".into()));
    assert_eq!(harness.state().scene.node(root).children.len(), before + 1, "Ctrl+V pasted the copied node");

    event(&mut harness, egui::Event::Cut);
    assert_eq!(harness.state().scene.node(root).children.len(), before, "Ctrl+X took the node away");
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
fn a_pattern_grip_points_the_way_it_slides_before_it_is_grabbed() {
    // The grips all asked for the left-right arrow, whichever way their own run
    // ran. Hovered here through the real viewport, so what is checked is the
    // cursor the window would actually show.
    let mut harness = harness_configured("grip-cursor", |app| {
        app.run(simple3d_core::keymap::Command::Pattern);
    });
    let pattern = harness.state().primary().expect("the tool leaves the pattern selected");
    harness.step();

    // Where the spacing grip is on screen, for a run along a given axis.
    let cursor_over = |harness: &mut Harness<'_, App>, step: Vec3| {
        {
            let params = harness.state_mut().scene.get_mut(pattern).unwrap().params_mut().unwrap();
            for (key, value) in [("step_x", step.x), ("step_y", step.y), ("step_z", step.z)] {
                params.insert(key.into(), simple3d_core::primitive::ParamValue::Length(value));
            }
        }
        // The grips are placed in the node's evaluated frame, so the pattern has
        // to have been evaluated once with these numbers before it offers any.
        let app = harness.state_mut();
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
        // The camera was framed on one shape; a run of copies reaches past it,
        // and a grip off the top of the viewport is a grip nothing can hover.
        app.frame_all();
        harness.step();
        let at = harness
            .state()
            .pattern_grips(pattern)
            .into_iter()
            .find(|g| g.label == "Spacing")
            .expect("a run of three copies has a spacing grip")
            .at;
        let screen = harness.state().current_view().project(at).expect("the grip is on screen").0;
        move_to(harness, screen);
        // Settled over several frames: a widget reports the frame it was drawn
        // in, so the first frame after the pointer moves is still the old one.
        for _ in 0..3 {
            harness.step();
        }
        harness.output().platform_output.cursor_icon
    };

    // A run straight up the world stands up on screen too, whatever the camera
    // is doing about the horizontal.
    assert_eq!(
        cursor_over(&mut harness, Vec3::new(0.0, 0.0, 20.0)),
        egui::CursorIcon::ResizeVertical,
        "a pattern stepping upward asked for a sideways arrow"
    );
    // And a run along the ground does not: whichever of the other three it is,
    // it is not the vertical one.
    let flat = cursor_over(&mut harness, Vec3::new(20.0, 0.0, 0.0));
    assert_ne!(flat, egui::CursorIcon::ResizeVertical, "a run along the ground asked for the upright arrow");
    assert!(
        matches!(
            flat,
            egui::CursorIcon::ResizeHorizontal | egui::CursorIcon::ResizeNwSe | egui::CursorIcon::ResizeNeSw
        ),
        "a grip that slides asked for {flat:?}"
    );
}

#[test]
fn a_shape_is_dragged_out_of_the_palette_and_dropped_into_the_tree() {
    // The palette's tiles add a shape at the document's insertion point when
    // they are clicked. Dragged, they carry the shape into the outliner and the
    // drop says which row it belongs on -- the same gesture, the same slab on
    // the pointer and the same drop indicator as dragging a row already there.
    let mut harness = harness("palette-drag");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let group = harness.state_mut().scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
    harness.state_mut().select_only(plate);
    harness.step();
    harness.step();
    let before = harness.state().scene.node(group).children.len();

    let tile = rect_of(&harness, crate::panel_primitives::tile_id("sphere")).center();
    press(&mut harness, tile);
    move_to(&mut harness, tile + egui::vec2(0.0, 6.0));
    // Settled over several frames: a response reports the frame it was drawn
    // in, so the press only becomes a drag a frame or two after the move.
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(
        harness.state().outliner_drag,
        Some(crate::app::Carried::Shape("sphere")),
        "dragging a tile did not pick the shape up"
    );

    // Over the group, held for several frames, because a response reports the
    // frame it was drawn in.
    for _ in 0..3 {
        let onto = rect_of(&harness, crate::panel_outliner::row_id(group)).center();
        move_to(&mut harness, onto);
        harness.step();
    }
    let target = harness.state().drop_target.expect("a shape from the palette marked no drop target");
    assert_eq!(target.into, Some(group), "the group under the pointer was not marked as what would take the drop");

    let onto = rect_of(&harness, crate::panel_outliner::row_id(group)).center();
    release(&mut harness, onto);
    harness.step();
    let app = harness.state();
    assert_eq!(app.scene.node(group).children.len(), before + 1, "the shape did not land in the group");
    let landed = *app.scene.node(group).children.last().unwrap();
    assert_eq!(app.scene.node(landed).spec().map(|s| s.type_id), Some("sphere"), "the wrong shape landed");
    assert_eq!(app.primary(), Some(landed), "the dropped shape was not left selected");
    assert!(app.outliner_drag.is_none() && app.drop_target.is_none(), "the drag outlived the drop");

    // And it undoes in one step, like every other add.
    harness.state_mut().run(simple3d_core::keymap::Command::Undo);
    harness.step();
    assert_eq!(harness.state().scene.node(group).children.len(), before);
}

#[test]
fn a_tile_still_adds_its_shape_when_it_is_merely_clicked() {
    // The tile answers to both gestures, and teaching it to drag must not have
    // cost it the click it had before.
    let mut harness = harness("palette-click");
    let root = harness.state().scene.root();
    let before = harness.state().scene.node(root).children.len();

    let tile = rect_of(&harness, crate::panel_primitives::tile_id("sphere")).center();
    press(&mut harness, tile);
    release(&mut harness, tile);
    harness.step();
    let app = harness.state();
    assert_eq!(app.scene.node(root).children.len(), before + 1, "a click on a tile added nothing");
    assert!(app.outliner_drag.is_none(), "a click left a drag running");
}

#[test]
fn a_dialog_stops_the_mouse_as_well_as_the_keyboard() {
    // With About open, clicking a tile in the palette still added a shape,
    // while Ctrl+N did nothing: `handle_shortcuts` returned early on a modal
    // and nothing stopped the pointer. A dialog left open behind the main
    // window was an application whose shortcuts had silently stopped while the
    // document could still be edited by mouse.
    let mut harness = harness("dialog-blocks-pointer");
    let root = harness.state().scene.root();
    let before = harness.state().scene.node(root).children.len();
    let tile = rect_of(&harness, crate::panel_primitives::tile_id("sphere")).center();

    harness.state_mut().modal = crate::app::Modal::About;
    // Two frames: the backdrop has to be drawn once before its layer can win a
    // hit test, which egui settles at the end of the pass it was drawn in.
    harness.step();
    harness.step();

    press(&mut harness, tile);
    release(&mut harness, tile);
    harness.step();
    assert_eq!(
        harness.state().scene.node(root).children.len(),
        before,
        "a click reached the palette through an open dialog"
    );

    harness.state_mut().modal = crate::app::Modal::None;
    harness.step();
    harness.step();
    press(&mut harness, tile);
    release(&mut harness, tile);
    harness.step();
    assert_eq!(
        harness.state().scene.node(root).children.len(),
        before + 1,
        "closing the dialog did not give the palette its clicks back"
    );
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
fn a_point_row_keeps_all_three_of_its_fields_on_the_panel() {
    // Reported from the running application: dragging the dock in took the Z
    // field of the 3D cursor and of the view centre off the panel edge, while
    // the position and rotation rows above them broke onto three lines as they
    // should. Issue 57 again, in the two rows that were written after it: the
    // width for the three fields came from `available_width`, which in a wrapped
    // row is the width a *new* line would have -- the whole row, label column
    // included -- so each field was sized as though the label were not there and
    // the second and third were drawn past the panel's edge. Held against the
    // old code this test fails at the second field of a 230 px dock, which
    // reached 1404.7 with the window edge at 1400.
    //
    // Checked across the dock's whole range (`width_range(200..=620)` in
    // `dock`), because the failure was in the middle of it rather than at an end.
    for width in [200.0_f32, 230.0, 260.0, 290.0, 320.0, 420.0, 620.0] {
        // Nothing selected: the Document section, and with it the 3D cursor and
        // the view centre, is what the panel shows in place of an object's own
        // rows.
        let mut harness = harness_configured("point-row-width", |app| {
            app.settings.properties_width = width;
            app.selection.clear();
        });
        harness.step();
        let edge = harness.ctx.screen_rect().right();
        for row in ["3D cursor", "View centre"] {
            let fields: Vec<egui::Rect> = (0..3)
                .map(|axis| rect_of(&harness, crate::panel_properties::grip_id(&format!("{row}:{axis}"))))
                .collect();
            for (axis, field) in fields.iter().enumerate() {
                assert!(
                    field.right() <= edge - 4.0,
                    "at a panel {width} px wide, {row}:{axis} reaches {} with the window edge at {edge}",
                    field.right()
                );
                assert!(
                    field.left() >= 0.0 && field.width() >= 40.0,
                    "at a panel {width} px wide, {row}:{axis} is {field:?}"
                );
            }
            // And they are laid out as one thing or the other: three across on
            // one line, or one to a line down the left. A row that is half of
            // each is the shape the bug had.
            let across = fields.windows(2).all(|p| (p[1].top() - p[0].top()).abs() < 1.0);
            let stacked = fields.windows(2).all(|p| p[1].top() > p[0].top() && (p[1].left() - p[0].left()).abs() < 1.0);
            assert!(across || stacked, "at a panel {width} px wide, {row} is neither across nor stacked: {fields:?}");
        }
    }
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
        // And every row ends at the same place. The unit is in the row's name
        // now rather than beside the field, so nothing takes a bite out of one
        // row's field that it does not take out of the others -- which is the
        // whole reason for writing it there. Measured on the last field of each
        // row, since the transform rows put three side by side.
        let last: Vec<&(String, egui::Rect)> =
            fields.iter().filter(|(name, _)| !name.contains(':') || name.ends_with(":2")).collect();
        let right = last[0].1.right();
        for (name, rect) in &last {
            assert!(
                (rect.right() - right).abs() < 1.0,
                "at a panel {width} px wide, {name} ends at {} and {} ends at {right}",
                rect.right(),
                last[0].0
            );
        }
        // The rows with one field to a line are that same width as each other,
        // whether their value carries a unit or not.
        let single: Vec<f32> =
            fields.iter().filter(|(name, _)| !name.contains(':')).map(|(_, rect)| rect.width()).collect();
        assert!(
            single.iter().all(|w| (w - single[0]).abs() < 1.0),
            "at a panel {width} px wide the dimension fields are different widths: {single:?}"
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

/// Issue 76 and issue 68 together, through the window: holding the snap key --
/// Ctrl on its own, which is only bindable at all because of issue 76 -- during
/// a move drag is what makes the drag snap to another body's geometry.
///
/// Every other test of this reaches one half of it: `geometry_snap_wanted` is
/// asked directly whether Ctrl means snap, and the drag arithmetic is driven
/// with `snap_requested` already set. Nothing until this drove a hand holding
/// Ctrl and dragging, which is the way anybody actually meets the feature.
#[test]
fn holding_the_snap_key_through_a_drag_snaps_to_another_body() {
    let mut harness = harness_configured("snap-while-held", |app| {
        app.settings.geometry_snap = simple3d_core::config::SnapMode::WhileHeld;
        // Two boxes rather than the starting plate: a small body to carry, and
        // one to snap it to, well clear of it so the target is unambiguous. The
        // grid step is coarse so a grid landing and a geometry landing cannot be
        // the same number by accident.
        let root = app.scene.root();
        for id in app.scene.node(root).children.clone() {
            app.scene.remove(id);
        }
        let carried = app.scene.add_primitive("box", root, 0).expect("the box is in the registry");
        let target = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
        app.scene.get_mut(target).unwrap().position = Vec3::new(75.0, 0.0, 0.0);
        app.scene.settings.snap_step = 10.0;
        app.select_only(carried);
    });
    let carried = harness.state().primary().unwrap();
    harness.state_mut().mode = crate::gizmo::Mode::Move;
    harness.step();
    harness.state_mut().evaluated =
        Evaluator::new().evaluate(&harness.state().scene, &simple3d_core::eval::Cancel::new());
    harness.state_mut().frame_all();
    harness.step();

    let view = harness.state().current_view();
    let gizmo = harness.state().gizmo_for(carried).expect("a gizmo for the selected box");
    let handle = crate::gizmo::Handle::MoveAxis(0);
    let (at, _) = view.project(gizmo.handle_point(handle, &view)).expect("the X arrow is off screen");

    modifiers(&mut harness, egui::Modifiers::COMMAND);
    press(&mut harness, at);
    move_to(&mut harness, at + egui::vec2(12.0, 0.0));
    // Dragged across the other box a step at a time: somewhere along the way the
    // pointer passes one of its corners, and that is what the carried box lands
    // on.
    let mut snapped = false;
    let mut landed = Vec3::ZERO;
    for step in 1..=16 {
        let to = view.project(gizmo.handle_point(handle, &view) + gizmo.axes[0] * (step as f64 * 6.0)).unwrap().0;
        move_to(&mut harness, to);
        if harness.state().snap_indicator.is_some() {
            snapped = true;
            landed = harness.state().scene.node(carried).position;
        }
    }
    assert!(harness.state().snap_requested, "Ctrl held through the drag did not ask for a geometry snap");
    release(&mut harness, at + egui::vec2(300.0, 0.0));
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();

    assert!(snapped, "the drag never met a feature of the other body to snap to");
    // The grid step is 10 mm, so a landing off it is one only the geometry can
    // have chosen.
    assert!(
        (landed.x / 10.0).fract().abs() > 1e-6,
        "the drag landed on the grid step at {landed:?}, so nothing snapped to the body"
    );
}

/// Issue 68, the whole point of it: two bodies brought face to face.
///
/// Placing a part against another is what geometry snapping is *for*, and it is
/// the case the feature could not do. The target used to be whatever feature the
/// *pointer* was over, and the manipulator handle is grabbed some seventy pixels
/// out from the body -- so by the time the pointer reached the corner to meet,
/// the body it was carrying had already been dragged on top of that corner. Two
/// 20 mm boxes could be snapped into the same 20 mm of space, and into nothing
/// else: the landing where their faces touch was never once offered.
///
/// The carried box starts at the origin and the target sits at 75, so the two
/// stand 55 mm apart with a 20 mm box between them. Their faces meet when the
/// carried box is at 55, which is the number this drag has to be able to reach.
#[test]
fn a_snapped_drag_can_put_two_boxes_face_to_face() {
    let mut harness = harness_configured("snap-face-to-face", |app| {
        app.settings.geometry_snap = simple3d_core::config::SnapMode::WhileHeld;
        let root = app.scene.root();
        for id in app.scene.node(root).children.clone() {
            app.scene.remove(id);
        }
        let carried = app.scene.add_primitive("box", root, 0).expect("the box is in the registry");
        let target = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
        app.scene.get_mut(target).unwrap().position = Vec3::new(75.0, 0.0, 0.0);
        // A grid step that cannot land on 55 by itself, so a landing there is one
        // the geometry chose.
        app.scene.settings.snap_step = 10.0;
        app.select_only(carried);
    });
    let carried = harness.state().primary().unwrap();
    harness.state_mut().mode = crate::gizmo::Mode::Move;
    harness.step();
    harness.state_mut().evaluated =
        Evaluator::new().evaluate(&harness.state().scene, &simple3d_core::eval::Cancel::new());
    harness.state_mut().frame_all();
    harness.step();

    let view = harness.state().current_view();
    let gizmo = harness.state().gizmo_for(carried).expect("a gizmo for the selected box");
    let handle = crate::gizmo::Handle::MoveAxis(0);
    let start = gizmo.handle_point(handle, &view);
    let (at, _) = view.project(start).expect("the X arrow is off screen");

    modifiers(&mut harness, egui::Modifiers::COMMAND);
    press(&mut harness, at);
    // Crossed the whole gap a millimetre at a time, keeping every place the drag
    // snapped to. A person doing this by eye stops at the one they wanted; the
    // test only has to prove it was offered at all.
    let mut landings: Vec<f64> = Vec::new();
    for step in 1..=90 {
        let to = view.project(start + gizmo.axes[0] * step as f64).expect("the drag ran off screen").0;
        move_to(&mut harness, to);
        if harness.state().snap_indicator.is_some() {
            landings.push(harness.state().scene.node(carried).position.x);
        }
    }
    release(&mut harness, at);
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();

    assert!(
        landings.iter().any(|x| (x - 55.0).abs() < 1e-6),
        "the drag never offered the landing where the two boxes touch; it snapped to {landings:?}"
    );
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
fn the_split_panels_button_joins_the_pieces_back_together_without_taking_the_panel_with_it() {
    use egui_kittest::kittest::Queryable;
    use simple3d_core::keymap::Command;

    // The button takes a node out of the tree, and the sections under it -- the
    // transform, the measurements -- are laid out in the same frame from the
    // selection that frame started with. Run on the spot, the click left them
    // drawing a row that was no longer there and the window went down with
    // "node 6 is not in the scene". So the panel asks for the command and the
    // command runs when the panel is done.
    let mut harness = harness_configured("split-panel", |app| {
        let root = app.scene.root();
        let group = app.scene.add_group(simple3d_core::scene::GroupOp::Union, root, 0);
        for i in 0..2 {
            let id = app.scene.add_primitive("box", group, i).unwrap();
            app.scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 60.0, 0.0, 0.0);
        }
        app.select_only(group);
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
        app.run(Command::BreakApart);
    });
    let split = harness.state().primary().expect("the split is selected");
    assert!(harness.state().scene.node(split).is_split());

    let label = format!("Join back together ({})", harness.state().keymap.shortcut_text(Command::Rejoin));
    assert!(harness.query_by_label(&label).is_some(), "the split panel has no button to join the pieces back");
    harness.get_by_label(&label).click();
    harness.step();
    harness.step();

    assert!(!harness.state().scene.contains(split), "the split is still there");
    let back = harness.state().primary().expect("the object that came back is selected");
    assert!(harness.state().scene.node(back).is_group(), "what came back is not the group it was made from");
    assert_eq!(harness.state().scene.node(back).children.len(), 2, "the operands did not come back");
}

/// The split tool end to end, through its real window: pick a cell shape, press
/// Split, and wait for the pieces the thread cuts to land in the document
/// (issue 82).
#[test]
fn the_split_tool_cuts_the_shape_into_the_cells_its_window_was_asked_for() {
    use egui_kittest::kittest::Queryable;
    use simple3d_core::keymap::Command;

    let mut harness = harness_configured("split-tool", |app| {
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    });
    harness.state_mut().run(Command::SplitIntoPieces);
    // A few frames rather than one: the window sizes itself over several, and a
    // click aimed at where a widget was before it settled lands on nothing.
    for _ in 0..6 {
        harness.step();
    }
    assert_eq!(harness.state().modal, crate::app::Modal::SplitTool, "the tool did not open");

    harness.get_by_label("Hexagons").click();
    harness.step();
    harness.step();
    assert_eq!(
        harness.state().split_tool.as_ref().unwrap().tiling.kind,
        simple3d_geom::tiling::CellKind::Hexagons,
        "clicking the cell shape did not choose it"
    );

    harness.get_by_label("Split").click();
    harness.step();
    harness.step();
    assert!(harness.state().split_job.is_some(), "pressing Split did not start the cutting");
    assert_eq!(harness.state().modal, crate::app::Modal::None, "the window stayed open over the cutting");

    // The cutting is on a thread of its own, and the frame loop is what carries
    // the answer back -- `App::update` in the application, and this in a harness
    // that drives `App::ui` alone.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while harness.state().split_job.is_some() {
        harness.state_mut().poll_split();
        harness.step();
        assert!(std::time::Instant::now() < deadline, "the split never landed");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    let split = harness.state().primary().expect("the split is selected");
    assert!(harness.state().scene.node(split).is_split(), "no split was made");
    assert!(harness.state().scene.node(split).children.len() > 1, "the shape came back in one piece");
    assert_eq!(
        harness.state().scene.node(split).split_tiling().map(|t| t.kind),
        Some(simple3d_geom::tiling::CellKind::Hexagons),
        "the pattern the pieces were cut with was not kept"
    );
    // And the panel of the split it made offers the way to cut it again.
    harness.step();
    harness.step();
    assert!(harness.query_by_label("Split differently\u{2026}").is_some(), "the split's panel offers no way to re-cut");
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

/// Deleting a pattern from the tree took the window with it.
///
/// The row list is taken before any of it is drawn, and the context menu's
/// Delete removes nodes while the loop over that list is still running. A
/// pattern is not a *group*, so deleting one asks nothing and goes at once --
/// and `Scene::remove` takes the whole subtree, which is exactly the rows that
/// come next. The next row drawn then asked the scene for a node that was no
/// longer in it, and `Scene::node` panics on that.
///
/// Driven through the real tree because that is the only place the fault
/// existed: every piece of it -- the list, the menu, the delete -- is correct on
/// its own, and it is the order they run in that was wrong.
#[test]
fn deleting_a_pattern_from_the_tree_does_not_take_the_window_with_it() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("outliner-delete-pattern");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    // A pattern *of* the plate, so the tree is Scene > Pattern > Plate and the
    // rows below the one being deleted are the ones that go with it.
    harness.state_mut().run(simple3d_core::keymap::Command::Pattern);
    harness.step();
    let pattern = harness.state().primary().expect("the plate is now inside a pattern");
    assert!(harness.state().scene.node(pattern).is_pattern());
    assert_eq!(harness.state().scene.node(pattern).children, vec![plate], "the plate is not in the pattern");

    let row = rect_of(&harness, crate::panel_outliner::row_id(pattern));
    let at = row.center();
    move_to(&mut harness, at);
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    harness.step();
    harness.step();

    // The menu's Delete carries its binding after a tab, so it is found by what
    // it starts with rather than by the whole label.
    let delete = harness
        .query_all_by_label_contains("Delete")
        .next()
        .expect("the tree's context menu has no Delete on a pattern");
    delete.click();
    // The frame the deletion happens on is the frame that used to panic: the
    // rows after the pattern are its children, and they are gone by then.
    harness.step();
    harness.step();

    assert!(!harness.state().scene.contains(pattern), "the pattern is still there");
    assert!(!harness.state().scene.contains(plate), "the pattern went and left its child behind");
    assert!(harness.state().selection.is_empty(), "the deleted node is still selected");
}

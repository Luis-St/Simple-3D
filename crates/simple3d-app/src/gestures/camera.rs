//! Moving the camera with the mouse.

use super::*;
use crate::app::App;
use egui_kittest::Harness;

// -- navigating: the middle button moves about the ground ---------------------

/// A drag on a named button, which `drag` above cannot do: it presses the
/// primary one, because that is what every gesture before this one used.
pub(crate) fn drag_button(
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
pub(crate) fn a_middle_drag_on_the_viewport_moves_the_camera_over_the_ground() {
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
pub(crate) fn wheel(harness: &mut Harness<'_, App>, at: egui::Pos2, delta: egui::Vec2) {
    move_to(harness, at);
    let modifiers = harness.input().modifiers;
    event(harness, egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, modifiers });
}

/// The other half of issue 72: the wheel zooms about the pointer rather than
/// about the middle of the frame, so what is under the cursor stays under it and
/// the wheel alone carries the view across the grid -- and, since issue 97, only
/// while the zoom hold is down, because a wheel that moves the view centre by
/// itself is a view centre that drifts off whatever it was set to. Driven
/// through the real panel, wheel event and all, rather than by calling
/// `apply_zoom`.
#[test]
pub(crate) fn a_scroll_over_the_viewport_zooms_about_the_pointer() {
    let mut harness = harness("viewport-zoom");
    let rect = harness.state().viewport_rect;
    // Clear of the view cube in the corner, which takes the pointer first.
    let at = rect.center() + egui::vec2(-150.0, 90.0);
    let before = harness.state().scene.camera;

    // Nothing held: the frame closes in on its own centre and leaves it alone.
    wheel(&mut harness, at, egui::vec2(0.0, 60.0));
    let plain = harness.state().scene.camera;
    assert!(
        plain.distance < before.distance,
        "scrolling up did not zoom in: {} -> {}",
        before.distance,
        plain.distance
    );
    assert_eq!(plain.target, before.target, "the plain wheel walked the view centre towards the pointer");

    // Held, it walks towards the pointer. Alt is the default hold; the binding
    // is read live, so this is the same thing a rebound key would do.
    let before = plain;
    // The world point the cursor is over. Parallel projection, so any point
    // along its ray is the same pixel and this needs nothing to be there.
    let under = crate::view::View::new(before, rect).ray(at).0;
    modifiers(&mut harness, egui::Modifiers::ALT);
    wheel(&mut harness, at, egui::vec2(0.0, 60.0));
    modifiers(&mut harness, egui::Modifiers::NONE);

    let after = harness.state().scene.camera;
    assert!(
        after.distance < before.distance,
        "scrolling up did not zoom in: {} -> {}",
        before.distance,
        after.distance
    );
    assert_eq!((after.yaw, after.pitch), (before.yaw, before.pitch), "the wheel turned the camera as well");
    assert_ne!(after.target, before.target, "the held zoom held the frame centre still instead of the pointer");

    let landed = crate::view::View::new(after, rect).project(under).unwrap().0;
    assert!((landed - at).length() < 1.0, "the point under the pointer slid from {at:?} to {landed:?}");
}

/// Which way an orbit drag turns the picture. Nothing pinned this before, which
/// is how it came to be argued over rather than read off: the drag carries the
/// model with the pointer, so the eye goes the other way along the screen's own
/// axes -- the same rule the pan is asserted by just above. The assertion is on
/// the *eye*, in the basis the drag was measured in, so it says what the gesture
/// means rather than which way a number happened to move.
#[test]
pub(crate) fn an_orbit_drag_carries_the_model_with_the_pointer() {
    let mut harness = harness("viewport-orbit-direction");
    let rect = harness.state().viewport_rect;
    // Clear of the view cube in the corner, which takes the pointer first.
    let from = rect.center() + egui::vec2(-40.0, 40.0);
    let before = harness.state().scene.camera;

    // Left, and a little down.
    drag_button(&mut harness, egui::PointerButton::Secondary, from, from + egui::vec2(-120.0, 40.0), 6);

    let after = harness.state().scene.camera;
    let view = crate::view::View::new(before, rect);
    let (right, up) = view.basis();
    let moved = crate::view::View::new(after, rect).eye() - view.eye();
    assert!(moved.dot(right) > 0.0, "dragging left did not turn the model left: {moved:?}");
    assert!(moved.dot(up) > 0.0, "dragging down did not lift the camera over the model: {moved:?}");
    assert_eq!(after.target, before.target, "orbiting moved the camera over the ground");
}

/// Issue 102: the manipulator wobbled around the shape while the camera turned,
/// because the picture was rasterized at the top of the viewport's frame and the
/// overlay drawn at the bottom of it -- one frame of orbit apart, and a frame is
/// however long the rasterizer happened to take. What is on screen cannot be
/// measured from here, but the cause can: at the end of a frame that orbited,
/// the picture the panel is showing must already be the one this camera asks
/// for. The drag is left *running*, because the frame after a release orbits
/// nothing and would agree either way.
#[test]
pub(crate) fn the_picture_is_drawn_from_the_camera_the_frames_own_orbit_left_behind() {
    let mut harness = harness("viewport-frame-camera");
    let rect = harness.state().viewport_rect;
    // Clear of the view cube in the corner, which takes the pointer first.
    let from = rect.center() + egui::vec2(-120.0, 40.0);
    let before = harness.state().scene.camera;

    move_to(&mut harness, from);
    button(&mut harness, from, egui::PointerButton::Secondary, true);
    move_to(&mut harness, from + egui::vec2(50.0, 30.0));
    move_to(&mut harness, from + egui::vec2(100.0, 60.0));
    assert_ne!(harness.state().scene.camera.yaw, before.yaw, "the drag did not orbit at all");

    let pixels_per_point = harness.ctx.pixels_per_point();
    let size = [
        (rect.width() * pixels_per_point).round().max(1.0) as usize,
        (rect.height() * pixels_per_point).round().max(1.0) as usize,
    ];
    let dark = harness.ctx.style().visuals.dark_mode;
    let app = harness.state();
    assert_eq!(
        app.image_key,
        crate::panel_viewport::image_key(app, size, dark),
        "the picture on screen was rasterized from a camera this frame's own orbit has already moved"
    );
}

/// The pattern tool's preview is a viewport too, and it navigates on the very
/// same bindings -- so it moves off the origin with the same drag (issue 72).
#[test]
pub(crate) fn a_middle_drag_on_the_pattern_previews_picture_moves_it_over_the_ground() {
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

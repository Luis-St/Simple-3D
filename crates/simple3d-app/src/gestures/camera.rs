//! Moving the camera with the mouse.

use super::*;
use crate::app::App;
use egui_kittest::Harness;

// -- navigating: the middle button moves about the ground ---------------------

/// A drag with a named button; `drag` always presses the primary one.
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

/// Issue 72: pan is the middle button, since orbit and zoom keep the target still and pan was
/// hidden behind Shift. Driven through the real panel.
#[test]
pub(crate) fn a_middle_drag_on_the_viewport_moves_the_camera_over_the_ground() {
    let mut harness = harness("viewport-pan");
    let rect = harness.state().viewport_rect;
    // Clear of the view cube, which takes the pointer first.
    let from = rect.center() + egui::vec2(-120.0, 40.0);
    let to = from + egui::vec2(140.0, 90.0);
    let before = harness.state().scene.camera;

    drag_button(&mut harness, egui::PointerButton::Middle, from, to, 6);

    let after = harness.state().scene.camera;
    assert_ne!(after.target, before.target, "a middle drag did not move the camera over the ground");
    // Pan only: no orbit or zoom along with it.
    assert_eq!((after.yaw, after.pitch), (before.yaw, before.pitch), "the middle drag orbited as well");
    assert_eq!(after.distance, before.distance, "the middle drag zoomed as well");

    // Dragging right and down carries the scene that way, so the camera moves the other way.
    let view = crate::view::View::new(before, rect);
    let (right, up) = view.basis();
    let moved = after.target - before.target;
    assert!(moved.dot(right) < 0.0, "dragging right did not carry the ground right: {moved:?}");
    assert!(moved.dot(up) > 0.0, "dragging down did not carry the ground down: {moved:?}");

    // The right button still orbits, without a modifier.
    let turned = harness.state().scene.camera;
    drag_button(&mut harness, egui::PointerButton::Secondary, from, to, 6);
    let orbited = harness.state().scene.camera;
    assert_ne!(orbited.yaw, turned.yaw, "the right button stopped orbiting");
    assert_eq!(orbited.target, turned.target, "orbiting moved the camera over the ground");
}

/// A wheel turn over the viewport, as the window gets it.
pub(crate) fn wheel(harness: &mut Harness<'_, App>, at: egui::Pos2, delta: egui::Vec2) {
    move_to(harness, at);
    let modifiers = harness.input().modifiers;
    event(harness, egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, modifiers });
}

/// Issue 72: the wheel zooms about the pointer, but since issue 97 only while the zoom hold is down,
/// so the view centre does not drift. Driven through the real panel.
#[test]
pub(crate) fn a_scroll_over_the_viewport_zooms_about_the_pointer() {
    let mut harness = harness("viewport-zoom");
    let rect = harness.state().viewport_rect;
    // Clear of the view cube, which takes the pointer first.
    let at = rect.center() + egui::vec2(-150.0, 90.0);
    let before = harness.state().scene.camera;

    // Nothing held: zoom about the frame centre, leaving it alone.
    wheel(&mut harness, at, egui::vec2(0.0, 60.0));
    let plain = harness.state().scene.camera;
    assert!(
        plain.distance < before.distance,
        "scrolling up did not zoom in: {} -> {}",
        before.distance,
        plain.distance
    );
    assert_eq!(plain.target, before.target, "the plain wheel walked the view centre towards the pointer");

    // Held (Alt by default, read live), it walks towards the pointer.
    let before = plain;
    // The world point under the cursor; parallel projection needs nothing to be there.
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

/// Which way an orbit drag turns the picture: the model follows the pointer, so the eye moves the
/// other way along the screen axes, as for pan. Asserted on the eye in the drag's basis.
#[test]
pub(crate) fn an_orbit_drag_carries_the_model_with_the_pointer() {
    let mut harness = harness("viewport-orbit-direction");
    let rect = harness.state().viewport_rect;
    // Clear of the view cube, which takes the pointer first.
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

/// Issue 102: the manipulator wobbled because the picture was rasterised a frame of orbit before the
/// overlay. At the end of an orbiting frame the shown picture must match the current camera. The
/// drag is left running, since a post-release frame would agree either way.
#[test]
pub(crate) fn the_picture_is_drawn_from_the_camera_the_frames_own_orbit_left_behind() {
    let mut harness = harness("viewport-frame-camera");
    let rect = harness.state().viewport_rect;
    // Clear of the view cube, which takes the pointer first.
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

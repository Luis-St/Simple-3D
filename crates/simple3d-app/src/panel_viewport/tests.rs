use super::cursor::*;
use super::gesture::*;
use super::navigate::*;
use crate::view::View;
use simple3d_core::keymap::MouseButton;
use simple3d_core::scene::Camera;

use simple3d_core::keymap::{Drag as NavDrag, Keymap};

#[test]
fn a_sliding_handle_points_the_way_it_actually_slides() {
    use egui::CursorIcon::*;
    // Every grip used to claim it slid left and right, whichever way its own
    // run ran -- a pattern stepping straight up the screen still asked for
    // the horizontal arrow.
    let cursor = |x: f32, y: f32| slide_cursor(egui::vec2(x, y));
    assert_eq!(cursor(1.0, 0.0), ResizeHorizontal);
    assert_eq!(cursor(0.0, 1.0), ResizeVertical);
    // Screen y grows downward, so right-and-down is the "\\" diagonal and
    // right-and-up is the "/" one.
    assert_eq!(cursor(1.0, 1.0), ResizeNwSe);
    assert_eq!(cursor(1.0, -1.0), ResizeNeSw);

    // A line has no sense of direction: pushing and pulling along it are the
    // same gesture, so the opposite bearing gives the same cursor.
    for (x, y) in [(1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, -1.0), (3.0, 1.0), (-1.0, 4.0)] {
        assert_eq!(cursor(x, y), cursor(-x, -y), "({x}, {y}) and its opposite disagree");
    }

    // The sectors meet where they should: just off the axis is still the
    // axis, and past the halfway line is the diagonal.
    assert_eq!(cursor(10.0, 3.0), ResizeHorizontal, "17 degrees off flat is still flat");
    assert_eq!(cursor(10.0, 6.0), ResizeNwSe, "31 degrees off flat is the diagonal");
    assert_eq!(cursor(3.0, 10.0), ResizeVertical);

    // A line that does not project -- edge on, or off the screen -- falls
    // back rather than picking a direction out of nothing.
    assert_eq!(cursor(0.0, 0.0), ResizeHorizontal);
}

fn only(button: MouseButton) -> [bool; 3] {
    [button == MouseButton::Left, button == MouseButton::Middle, button == MouseButton::Right]
}

fn view_of(camera: Camera) -> View {
    View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)))
}

/// Spec acceptance criterion 29: remap the orbit mouse button and navigation
/// follows the new binding immediately, without a restart.
///
/// "Without a restart" is the load-bearing half, and it is a property of
/// where the binding is read from: `navigate` passes the live `NavMap` in on
/// every frame rather than caching one. The test mutates the same keymap it
/// already queried and asserts the answer changes.
#[test]
fn remapping_the_orbit_button_takes_effect_without_a_restart() {
    let mut keymap = Keymap::default();
    let original = keymap.nav.orbit;
    assert_eq!(nav_gesture(&keymap.nav, only(original.button), false, false, false), Some(Gesture::Orbit));

    // Pick a button the default map does not already use for orbit.
    let remapped = [MouseButton::Left, MouseButton::Middle, MouseButton::Right]
        .into_iter()
        .find(|&b| b != original.button && !keymap.nav.pan.matches(b, false, false, false))
        .expect("a free button");
    keymap.nav.orbit = NavDrag::new(remapped);

    // The very next drag follows the new binding -- no reload of anything.
    assert_eq!(nav_gesture(&keymap.nav, only(remapped), false, false, false), Some(Gesture::Orbit));
    assert_ne!(
        nav_gesture(&keymap.nav, only(original.button), false, false, false),
        Some(Gesture::Orbit),
        "the old button still orbits"
    );

    // And the camera really moves on the new binding, through the same call
    // `navigate` makes.
    let mut camera = Camera::default();
    let before = camera.yaw;
    let gesture = nav_gesture(&keymap.nav, only(remapped), false, false, false).unwrap();
    let view = view_of(camera);
    apply_gesture(&mut camera, gesture, egui::vec2(30.0, 0.0), &view);
    assert_ne!(camera.yaw, before, "orbiting on the new binding did not turn the camera");
}

/// The same immediacy for pan and for the invert-zoom switch, and the rule
/// that keeps pan and orbit apart when pan is orbit's chord plus a modifier.
#[test]
fn pan_wins_over_orbit_on_the_same_button_with_a_modifier() {
    let mut keymap = Keymap::default();
    keymap.nav.orbit = NavDrag::new(MouseButton::Right);
    keymap.nav.pan = NavDrag::with_shift(MouseButton::Right);

    assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Right), false, false, false), Some(Gesture::Orbit));
    assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Right), false, true, false), Some(Gesture::Pan));
    // A modifier neither binding asks for matches nothing, rather than
    // falling back to orbit.
    assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Right), true, false, false), None);
    assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Left), false, false, false), None);
    assert_eq!(nav_gesture(&keymap.nav, [false; 3], false, false, false), None);
}

#[test]
fn inverting_the_zoom_reverses_which_way_the_wheel_goes() {
    let mut keymap = Keymap::default();
    keymap.nav.invert_zoom = false;
    let mut normal = Camera::default();
    apply_zoom(&mut normal, &keymap.nav, 10.0, None);

    keymap.nav.invert_zoom = true;
    let mut inverted = Camera::default();
    apply_zoom(&mut inverted, &keymap.nav, 10.0, None);

    let start = Camera::default().distance;
    assert_ne!(normal.distance, start);
    assert!(
        (normal.distance - start).signum() != (inverted.distance - start).signum(),
        "inverting the zoom did not reverse it: {} vs {}",
        normal.distance,
        inverted.distance
    );
    // Wheel noise below the threshold does nothing at all.
    let mut still = Camera::default();
    apply_zoom(&mut still, &keymap.nav, 0.001, None);
    assert_eq!(still.distance, start);
}

/// The other half of moving freely about the grid (issue 72): the wheel
/// zooms about the pointer, so whatever is under it stays under it. Zooming
/// about the frame centre is what made the viewport read as bolted down --
/// a corner of the model you were closing in on slid off the side of the
/// frame, and it took a pan after every scroll to bring it back.
#[test]
fn zooming_keeps_whatever_is_under_the_pointer_under_it() {
    let keymap = Keymap::default();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
    let start = Camera { yaw: -55.0, pitch: 28.0, distance: 300.0, ..Camera::default() };

    for (yaw, pitch) in [(-55.0, 28.0), (-90.0, 0.0), (140.0, -35.0), (-90.0, 89.9)] {
        let start = Camera { yaw, pitch, ..start };
        for offset in [egui::vec2(-260.0, 150.0), egui::vec2(310.0, -210.0), egui::vec2(0.0, -320.0)] {
            let cursor = rect.center() + offset;
            // Any world point on the pointer's ray will do: the projection
            // is parallel, so every point along it lands on the same pixel.
            // The one this picks is on the plane through the eye, which is
            // sky rather than ground for an offset above the horizon --
            // exactly the case a raycast onto the ground would have had no
            // answer for.
            let (under, _) = View::new(start, rect).ray(cursor);
            for scroll in [120.0, -120.0] {
                let mut camera = start;
                apply_zoom(&mut camera, &keymap.nav, scroll, Some((rect, cursor)));
                assert_ne!(camera.distance, start.distance, "the wheel did not zoom at all");
                let after = View::new(camera, rect).project(under).unwrap().0;
                assert!(
                    (after - cursor).length() < 0.01,
                    "the point under the pointer slid from {cursor:?} to {after:?} \
                     (yaw {yaw}, pitch {pitch}, scroll {scroll})"
                );
            }
        }
    }

    // Without an anchor -- the pointer outside the panel, or a caller that
    // has no panel to speak of -- it is the zoom it always was, about the
    // frame centre.
    let cursor = rect.center() + egui::vec2(-260.0, 150.0);
    let mut centred = start;
    apply_zoom(&mut centred, &keymap.nav, 120.0, None);
    assert_ne!(centred.distance, start.distance);
    assert_eq!(centred.target, start.target, "zooming with no pointer to zoom about moved the camera");

    // At either end of the distance range the clamp holds the zoom still,
    // and the view has to hold still with it rather than sliding sideways
    // under a scroll that did nothing.
    let mut pinned = Camera { distance: 0.05, ..start };
    apply_zoom(&mut pinned, &keymap.nav, 120.0, Some((rect, cursor)));
    assert_eq!(pinned.distance, 0.05, "the near end of the range stopped holding");
    assert_eq!(pinned.target, start.target, "a zoom the clamp refused still moved the camera");

    // And a panel with no area is not a place to zoom about: the camera
    // keeps a target it can be steered from rather than being handed NaN.
    let mut sized = start;
    apply_zoom(&mut sized, &keymap.nav, 120.0, Some((egui::Rect::NOTHING, cursor)));
    assert_eq!(sized.target, start.target, "an empty panel moved the camera to {:?}", sized.target);
}

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
    // Regression: every grip showed the horizontal arrow, whichever way its run went.
    let cursor = |x: f32, y: f32| slide_cursor(egui::vec2(x, y));
    assert_eq!(cursor(1.0, 0.0), ResizeHorizontal);
    assert_eq!(cursor(0.0, 1.0), ResizeVertical);
    // Screen y grows downward, so right-and-down is the "\\" diagonal.
    assert_eq!(cursor(1.0, 1.0), ResizeNwSe);
    assert_eq!(cursor(1.0, -1.0), ResizeNeSw);

    // A line has no direction: the opposite bearing gives the same cursor.
    for (x, y) in [(1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, -1.0), (3.0, 1.0), (-1.0, 4.0)] {
        assert_eq!(cursor(x, y), cursor(-x, -y), "({x}, {y}) and its opposite disagree");
    }

    // Sector boundaries: near an axis is still the axis, past halfway is the diagonal.
    assert_eq!(cursor(10.0, 3.0), ResizeHorizontal, "17 degrees off flat is still flat");
    assert_eq!(cursor(10.0, 6.0), ResizeNwSe, "31 degrees off flat is the diagonal");
    assert_eq!(cursor(3.0, 10.0), ResizeVertical);

    // A line that does not project falls back rather than inventing a direction.
    assert_eq!(cursor(0.0, 0.0), ResizeHorizontal);
}

fn only(button: MouseButton) -> [bool; 3] {
    [button == MouseButton::Left, button == MouseButton::Middle, button == MouseButton::Right]
}

fn view_of(camera: Camera) -> View {
    View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0)))
}

/// Spec acceptance criterion 29: remapping the orbit button takes effect immediately, since
/// `navigate` reads the live `NavMap` every frame.
#[test]
fn remapping_the_orbit_button_takes_effect_without_a_restart() {
    let mut keymap = Keymap::default();
    let original = keymap.nav.orbit;
    assert_eq!(nav_gesture(&keymap.nav, only(original.button), false, false, false), Some(Gesture::Orbit));

    // A button the default map does not use for orbit.
    let remapped = [MouseButton::Left, MouseButton::Middle, MouseButton::Right]
        .into_iter()
        .find(|&b| b != original.button && !keymap.nav.pan.matches(b, false, false, false))
        .expect("a free button");
    keymap.nav.orbit = NavDrag::new(remapped);

    // The very next drag follows the new binding.
    assert_eq!(nav_gesture(&keymap.nav, only(remapped), false, false, false), Some(Gesture::Orbit));
    assert_ne!(
        nav_gesture(&keymap.nav, only(original.button), false, false, false),
        Some(Gesture::Orbit),
        "the old button still orbits"
    );

    // The camera really moves, through the same call `navigate` makes.
    let mut camera = Camera::default();
    let before = camera.yaw;
    let gesture = nav_gesture(&keymap.nav, only(remapped), false, false, false).unwrap();
    let view = view_of(camera);
    apply_gesture(&mut camera, gesture, egui::vec2(30.0, 0.0), &view);
    assert_ne!(camera.yaw, before, "orbiting on the new binding did not turn the camera");
}

/// The same for pan and invert-zoom, and pan staying distinct from orbit when it is orbit's chord
/// plus a modifier.
#[test]
fn pan_wins_over_orbit_on_the_same_button_with_a_modifier() {
    let mut keymap = Keymap::default();
    keymap.nav.orbit = NavDrag::new(MouseButton::Right);
    keymap.nav.pan = NavDrag::with_shift(MouseButton::Right);

    assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Right), false, false, false), Some(Gesture::Orbit));
    assert_eq!(nav_gesture(&keymap.nav, only(MouseButton::Right), false, true, false), Some(Gesture::Pan));
    // A modifier neither binding asks for matches nothing rather than falling back to orbit.
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
    // Wheel noise below the threshold does nothing.
    let mut still = Camera::default();
    apply_zoom(&mut still, &keymap.nav, 0.001, None);
    assert_eq!(still.distance, start);
}

/// The wheel zooms about the pointer, keeping what is under it in place (issue 72).
#[test]
fn zooming_keeps_whatever_is_under_the_pointer_under_it() {
    let keymap = Keymap::default();
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
    let start = Camera { yaw: -55.0, pitch: 28.0, distance: 300.0, ..Camera::default() };

    for (yaw, pitch) in [(-55.0, 28.0), (-90.0, 0.0), (140.0, -35.0), (-90.0, 89.9)] {
        let start = Camera { yaw, pitch, ..start };
        for offset in [egui::vec2(-260.0, 150.0), egui::vec2(310.0, -210.0), egui::vec2(0.0, -320.0)] {
            let cursor = rect.center() + offset;
            // Any point on the pointer's ray will do under parallel projection, even one in the sky, where a
            // ground raycast would have no answer.
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

    // Without an anchor it zooms about the frame centre.
    let cursor = rect.center() + egui::vec2(-260.0, 150.0);
    let mut centred = start;
    apply_zoom(&mut centred, &keymap.nav, 120.0, None);
    assert_ne!(centred.distance, start.distance);
    assert_eq!(centred.target, start.target, "zooming with no pointer to zoom about moved the camera");

    // At the distance clamp the view must not slide under a scroll that did nothing.
    let mut pinned = Camera { distance: 0.05, ..start };
    apply_zoom(&mut pinned, &keymap.nav, 120.0, Some((rect, cursor)));
    assert_eq!(pinned.distance, 0.05, "the near end of the range stopped holding");
    assert_eq!(pinned.target, start.target, "a zoom the clamp refused still moved the camera");

    // A zero-area panel keeps a finite target rather than NaN.
    let mut sized = start;
    apply_zoom(&mut sized, &keymap.nav, 120.0, Some((egui::Rect::NOTHING, cursor)));
    assert_eq!(sized.target, start.target, "an empty panel moved the camera to {:?}", sized.target);
}

/// A pan on a zero-height viewport does nothing. Unguarded, `mm_per_pixel_at` gave 1e9 mm per
/// pixel and a tiny pan threw the target billions of millimetres away.
#[test]
fn a_pan_with_no_viewport_to_measure_in_leaves_the_camera_alone() {
    for size in [egui::vec2(900.0, 0.0), egui::vec2(0.0, 700.0), egui::Vec2::ZERO] {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
        let start = Camera::default();
        let mut camera = start;
        apply_gesture(&mut camera, Gesture::Pan, egui::vec2(3.0, 2.0), &View::new(start, rect));
        assert_eq!(camera.target, start.target, "a pan across a {size:?} viewport moved the view centre");
    }

    // A pan across a real viewport still pans.
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
    let start = Camera::default();
    let mut camera = start;
    apply_gesture(&mut camera, Gesture::Pan, egui::vec2(3.0, 2.0), &View::new(start, rect));
    assert_ne!(camera.target, start.target, "a pan across a real viewport moved nothing");
}

//! Putting the 3D cursor somewhere, and the view centre beside it.

use super::*;
use simple3d_geom::Vec3;

// -- the 3D cursor ------------------------------------------------------------

#[test]
pub(crate) fn shift_right_click_puts_the_3d_cursor_where_the_pointer_is() {
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
pub(crate) fn shift_right_click_on_empty_space_puts_the_3d_cursor_back_at_the_origin() {
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
pub(crate) fn the_document_section_reads_and_sets_what_the_camera_looks_at() {
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
pub(crate) fn a_locked_view_centre_is_the_one_thing_that_does_not_move() {
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

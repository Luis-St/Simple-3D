//! Clicking a face of the orientation cube.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_geom::Vec3;

// -- the view cube ------------------------------------------------------------

/// Where the cube's front face is drawn *right now*, from the cube's own
/// projection -- the same function the drawing uses, so a test clicks what is
/// on screen rather than where it assumes it to be.
#[track_caller]
pub(crate) fn front_face(harness: &Harness<'_, App>) -> egui::Pos2 {
    let camera = harness.state().scene.camera;
    let cube = rect_of(harness, crate::panel_viewport::cube_id());
    let reach = crate::theme::metric::VIEW_CUBE * 0.30;
    let (normal, _, _) = crate::view::CUBE_FACES[front_index()];
    let n = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
    let (offset, depth) = crate::view::cube_project(camera.yaw, camera.pitch, n, reach);
    assert!(depth < 0.0, "the front face is turned away in this view; a click there would go through it");
    cube.center() + offset
}

pub(crate) fn front_index() -> usize {
    crate::view::CUBE_FACES.iter().position(|(_, _, label)| *label == "FRT").unwrap()
}

#[test]
pub(crate) fn clicking_a_face_of_the_view_cube_asks_for_the_view_that_face_shows() {
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
pub(crate) fn the_cube_takes_the_pointer_so_a_click_on_it_does_nothing_to_the_scene_behind_it() {
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

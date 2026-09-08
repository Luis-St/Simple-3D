//! Projecting to the screen and back.

use super::*;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn the_front_view_looks_along_positive_y() {
    let v = view();
    assert!(close(v.eye(), Vec3::new(0.0, -100.0, 0.0)), "{:?}", v.eye());
    assert!(close(v.forward(), Vec3::new(0.0, 1.0, 0.0)), "{:?}", v.forward());
    let (right, up) = v.basis();
    assert!(close(right, Vec3::new(1.0, 0.0, 0.0)), "{right:?}");
    assert!(close(up, Vec3::new(0.0, 0.0, 1.0)), "{up:?}");
}

#[test]
pub(crate) fn the_target_projects_to_the_centre_of_the_viewport() {
    let v = view();
    let (screen, depth) = v.project(v.camera.target).unwrap();
    assert!((screen - v.centre).length() < 1e-3, "{screen:?} vs {:?}", v.centre);
    assert!((depth - 100.0).abs() < 1e-6);
}

#[test]
pub(crate) fn screen_right_is_world_right_and_screen_up_is_world_up() {
    let v = view();
    let right = v.project(Vec3::new(10.0, 0.0, 0.0)).unwrap().0;
    assert!(right.x > v.centre.x, "+X should be to the right");
    let up = v.project(Vec3::new(0.0, 0.0, 10.0)).unwrap().0;
    assert!(up.y < v.centre.y, "+Z should be up (screen Y grows downward)");
}

#[test]
pub(crate) fn projecting_and_unprojecting_agree() {
    let mut v = view();
    v.camera.yaw = -55.0;
    v.camera.pitch = 28.0;
    for world in [Vec3::new(10.0, 5.0, 3.0), Vec3::new(-30.0, 12.0, -8.0), Vec3::new(0.0, 0.0, 0.0)] {
        let (screen, _) = v.project(world).unwrap();
        let (origin, dir) = v.ray(screen);
        // The world point must lie on the ray through its own projection.
        let along = (world - origin).dot(dir);
        let closest = origin + dir * along;
        // Screen coordinates are f32, so a round trip through them is
        // good to a fraction of a micrometre, not to the last bit.
        assert!((closest - world).length() < 1e-3, "{world:?} -> {screen:?} -> off by {}", (closest - world).length());
    }
}

#[test]
pub(crate) fn a_point_behind_the_eye_still_projects_where_it_belongs() {
    // There is no near plane to fall behind: the projection is parallel, so
    // a point the camera has passed lands at its true screen position and
    // is simply behind everything else. Nothing has to be clipped, which is
    // what stops geometry disappearing when the camera is inside the model.
    let v = view();
    let behind = Vec3::new(0.0, -200.0, 0.0);
    let (screen, depth) = v.project(behind).unwrap();
    assert!((screen - v.centre).length() < 1e-3, "{screen:?}");
    assert!(depth < 0.0, "a point behind the eye should have negative depth, got {depth}");
}

#[test]
pub(crate) fn an_axis_drag_solves_for_distance_along_the_axis() {
    let v = view();
    let origin = Vec3::ZERO;
    let axis = Vec3::new(1.0, 0.0, 0.0);
    let target = Vec3::new(25.0, 0.0, 0.0);
    let (screen, _) = v.project(target).unwrap();
    let along = v.ray_axis(screen, origin, axis).unwrap();
    assert!((along - 25.0).abs() < 1e-3, "got {along}");
}

#[test]
pub(crate) fn a_plane_drag_lands_on_the_plane() {
    let v = view();
    let hit = v.ray_plane(v.centre + egui::vec2(60.0, -30.0), Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)).unwrap();
    assert!(hit.y.abs() < 1e-6, "not on the plane: {hit:?}");
    assert!(hit.x > 0.0 && hit.z > 0.0, "{hit:?}");
}

#[test]
pub(crate) fn a_pixel_covers_more_millimetres_as_the_camera_pulls_back() {
    let mut v = view();
    let near = v.mm_per_pixel_at(Vec3::ZERO);
    v.camera.distance = 400.0;
    let far = v.mm_per_pixel_at(Vec3::ZERO);
    assert!(far > near * 3.5, "{near} -> {far}");
}

#[test]
pub(crate) fn looking_straight_down_still_yields_a_usable_basis() {
    let mut v = view();
    v.camera.pitch = 90.0;
    let (right, up) = v.basis();
    assert!(right.length() > 0.9 && up.length() > 0.9);
    assert!(right.dot(up).abs() < 1e-6, "basis is not orthogonal");
}
